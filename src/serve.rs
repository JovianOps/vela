//! vela serve — the UI, served by the same binary as the core.
//!
//!   vela serve [--port 8420] [--root <dir>]
//!
//! Serves the built UI (ui/dist) and a small JSON API backed by the Rust core:
//!   GET  /api/list?dir=        directory entries
//!   GET  /api/file?path=       file text
//!   POST /api/save             { path, text }
//!   GET  /api/highlight?path=&line=
    ///
// The browser never touches the filesystem: every read and write goes
// through the core, so the same rules (UTF-8, no path escaping the root) apply.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

pub fn serve(port: u16, root: PathBuf) -> std::io::Result<()> {
    let ui = find_ui_dir();
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("vela ui on http://127.0.0.1:{}", port);
    if let Some(dir) = &ui {
        println!("serving ui from {}", dir.display());
    } else {
        println!("ui/dist not found: run `npm install && npm run build` in ui/");
    }
    println!("root: {}", root.display());

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let root = root.clone();
        let ui = ui.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &root, ui.as_deref());
        });
    }
    Ok(())
}

fn find_ui_dir() -> Option<PathBuf> {
    // resolve next to the executable, not the working directory: a double-click
    // starts in an arbitrary folder
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    roots.push(PathBuf::from("."));

    for root in roots {
        for c in ["ui/dist", "ui"] {
            let p = root.join(c);
            if p.join("index.html").exists() {
                return Some(p);
            }
        }
    }
    None
}

fn handle(mut stream: TcpStream, root: &Path, ui: Option<&Path>) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");

    let path = target.split('?').next().unwrap_or("/");
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");

    let (status, ctype, body): (u16, &str, Vec<u8>) = if let Some(api) = path.strip_prefix("/api/") {
        api_route(api, query, &req, root)
    } else if method == "GET" {
        static_file(path, ui)
    } else {
        (405, "text/plain", b"method not allowed".to_vec())
    };

    let reason = if status == 200 { "OK" } else { "Error" };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status,
        reason,
        ctype,
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)?;
    Ok(())
}

fn api_route(api: &str, query: &str, req: &str, root: &Path) -> (u16, &'static str, Vec<u8>) {
    match api {
        "list" => {
            let dir = param(query, "dir").unwrap_or_else(|| ".".into());
            match list_dir(root, Path::new(&dir)) {
                Ok(entries) => (200, "application/json", entries.into_bytes()),
                Err(e) => (400, "text/plain", e.into_bytes()),
            }
        }
        "file" => {
            let p = param(query, "path").unwrap_or_default();
            match read_file(root, Path::new(&p)) {
                Ok(text) => (200, "application/json", json_str(&text).into_bytes()),
                Err(e) => (400, "text/plain", e.into_bytes()),
            }
        }
        "save" => {
            let body = req.split("\r\n\r\n").nth(1).unwrap_or("");
            match save_file(root, body) {
                Ok(()) => (200, "application/json", b"{\"ok\":true}".to_vec()),
                Err(e) => (400, "text/plain", e.into_bytes()),
            }
        }
        "highlight" => {
            let p = param(query, "path").unwrap_or_default();
            let line: usize = param(query, "line").unwrap_or_else(|| "0".into()).parse().unwrap_or(0);
            match read_file(root, Path::new(&p)) {
                Ok(text) => {
                    let spans = highlight_line(Path::new(&p), &text, line);
                    (200, "application/json", spans.into_bytes())
                }
                Err(e) => (400, "text/plain", e.into_bytes()),
            }
        }
        _ => (404, "text/plain", b"unknown api".to_vec()),
    }
}

fn highlight_line(path: &Path, text: &str, line: usize) -> String {
    let Some(lang) = crate::detect(&path.to_string_lossy()) else {
        return "[]".into();
    };
    let Some(src) = text.lines().nth(line) else {
        return "[]".into();
    };
    let items: Vec<String> = crate::highlight(lang, src)
        .into_iter()
        .map(|(k, t)| format!("{{\"k\":\"{}\",\"t\":{}}}", kind_str(k), json_str(&t)))
        .collect();
    format!("[{}]", items.join(","))
}

fn kind_str(k: crate::Kind) -> &'static str {
    match k {
        crate::Kind::Keyword => "keyword",
        crate::Kind::String => "string",
        crate::Kind::Comment => "comment",
        crate::Kind::Number => "number",
        crate::Kind::Plain => "plain",
    }
}

fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.replace('\\', "/");
    if rel.contains("..") {
        return Err("path may not contain '..'".into());
    }
    let p = root.join(&rel);
    if !p.starts_with(root) {
        return Err("path escapes the root".into());
    }
    Ok(p)
}

fn list_dir(root: &Path, rel: &Path) -> Result<String, String> {
    let dir = if rel.as_os_str().is_empty() { root.to_path_buf() } else { safe_join(root, &rel.to_string_lossy())? };
    let mut out = vec![format!("{{\"path\":\"{}\",\"dir\":true}}", "")];
    let rd = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut names: Vec<String> = vec![];
    for e in rd.flatten() {
        names.push(e.file_name().to_string_lossy().to_string());
    }
    names.sort();
    for n in names {
        let p = dir.join(&n);
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        if p.is_dir() {
            out.push(format!("{{\"path\":{},\"dir\":true}}", json_str(&rel)));
        } else {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            out.push(format!("{{\"path\":{},\"size\":{}}}", json_str(&rel), size));
        }
    }
    Ok(format!("[{}]", out[1..].join(",")))
}

fn read_file(root: &Path, rel: &Path) -> Result<String, String> {
    let p = if rel.as_os_str().is_empty() { return Err("no path".into()) } else { safe_join(root, &rel.to_string_lossy())? };
    std::fs::read_to_string(&p).map_err(|e| format!("{}: {}", p.display(), e))
}

fn save_file(root: &Path, body: &str) -> Result<(), String> {
    // body is JSON: {"path":"...","text":"..."}
    let path = json_field(body, "path").ok_or("missing path")?;
    let text = json_field(body, "text").unwrap_or_default();
    let p = safe_join(root, &path)?;
    std::fs::write(&p, text).map_err(|e| format!("{}: {}", p.display(), e))
}

fn param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        if k == key {
            let v = it.next().unwrap_or("");
            return Some(url_decode(v));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(b'?'));
                i += 3;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_field(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\":", key);
    let i = body.find(&pat)? + pat.len();
    let rest = &body[i..];
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let mut out = String::new();
        let mut it = stripped.chars();
        while let Some(c) = it.next() {
            match c {
                '\\' => match it.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(o) => out.push(o),
                    None => break,
                },
                '"' => break,
                c => out.push(c),
            }
        }
        Some(out)
    } else {
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.')).unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn static_file(path: &str, ui: Option<&Path>) -> (u16, &'static str, Vec<u8>) {
    let Some(dir) = ui else {
        return (404, "text/plain", b"ui not built".to_vec());
    };
    let rel = if path == "/" { "index.html".to_string() } else { path.trim_start_matches('/').to_string() };
    let p = dir.join(&rel);
    let p = if p.exists() { p } else { dir.join("index.html") };
    match std::fs::read(&p) {
        Ok(bytes) => {
            let ctype = match p.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript",
                Some("css") => "text/css",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                Some("woff2") => "font/woff2",
                _ => "application/octet-stream",
            };
            (200, ctype, bytes)
        }
        Err(_) => (404, "text/plain", b"not found".to_vec()),
    }
}

pub fn _unused(_: &mut dyn Write) {}