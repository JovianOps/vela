use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        // double-clicked: start the editor and open it, so the window does not
        // just flash and close
        let root = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let port = 8420u16;
        let url = format!("http://127.0.0.1:{}", port);
        println!("vela — starting the editor at {}", url);
        println!("root: {}", root.display());
        println!("(close this window to stop)");
        let _ = open_browser(&url);
        if let Err(e) = vela::serve::serve(port, root) {
            eprintln!("serve failed: {}", e);
            pause_on_error();
            std::process::exit(1);
        }
        return;
    }

    // a leading flag means "serve" (vela --port 9000 --root ./x)
    let cmd_owned: String;
    let cmd = if args[0].starts_with('-') && args[0] != "--help" && args[0] != "-h"
        && args[0] != "--version" && args[0] != "-v"
    {
        cmd_owned = "serve".to_string();
        cmd_owned.as_str()
    } else {
        args[0].as_str()
    };
    let rest: &[String] = if cmd == "serve" && args[0].starts_with('-') {
        &args[..]
    } else {
        &args[1..]
    };

    match cmd {
        "show" => {
            let path = need(rest, 0, "show <file>");
            let buf = load(path);
            let lang = kaelo_lang(path);
            for (i, line) in buf.lines.iter().enumerate() {
                let n = format!("{:>4} | ", i + 1);
                print!("{}", n);
                if let Some(l) = lang {
                    for (_, text) in vela::highlight(l, line) {
                        print!("{}", text);
                    }
                } else {
                    print!("{}", line);
                }
                println!();
            }
        }
        "stats" => {
            let path = need(rest, 0, "stats <file>");
            let buf = load(path);
            let s = buf.stats();
            println!("lines {}\nchars {}\nbytes {}", s.lines, s.chars, s.bytes);
        }
        "find" => {
            let pat = need(rest, 0, "find <pat> <file>");
            let path = need(rest, 1, "find <pat> <file>");
            let buf = load(path);
            match buf.find(pat, 0) {
                Some(i) => println!("{}: {}", i + 1, buf.line(i)),
                None => println!("not found"),
            }
        }
        "replace" => {
            let pat = need(rest, 0, "replace <pat> <with> <file>");
            let with = need(rest, 1, "replace <pat> <with> <file>");
            let path = need(rest, 2, "replace <pat> <with> <file>");
            let mut buf = load(path);
            let n = buf.replace(pat, with);
            buf.save(path).expect("save");
            println!("{} line(s) changed", n);
        }
        "serve" => {
            let mut port = 8420u16;
            let mut root = std::env::current_dir().unwrap_or_else(|_| ".".into());
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--port" => {
                        if let Some(p) = rest.get(i + 1) {
                            port = p.parse().unwrap_or(8420);
                        }
                        i += 2;
                    }
                    "--root" => {
                        if let Some(r) = rest.get(i + 1) {
                            root = r.into();
                        }
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            if let Err(e) = vela::serve::serve(port, root) {
                eprintln!("serve failed: {}", e);
                std::process::exit(1);
            }
        }
        "--version" | "-v" => println!("vela {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" => usage(),
        other => {
            println!("unknown command '{}'", other);
            usage();
        }
    }
    let _ = std::io::stdout().flush();
}

/// Open the default browser. `start` is a cmd builtin, so it goes through cmd.
fn open_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()?;
    Ok(())
}

/// Keep the window open long enough to read the error when double-clicked.
fn pause_on_error() {
    eprintln!("Press Enter to close...");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

fn usage() {
    println!(
        "vela {} — a small text editor core\\n\\n  vela show <file>              numbered, syntax-highlighted view\\n  vela stats <file>             lines, chars, bytes\\n  vela find <pat> <file>        first matching line\\n  vela replace <pat> <with> <file>  replace in place, prints the count\\n  vela serve [--port 8420] [--root <dir>]  serve the web UI and file API
  vela --version",
        env!("CARGO_PKG_VERSION")
    );
}

fn need<'a>(args: &'a [String], i: usize, usage: &str) -> &'a str {
    match args.get(i) {
        Some(s) => s.as_str(),
        None => {
            eprintln!("usage: vela {}", usage);
            std::process::exit(1);
        }
    }
}

fn load(path: &str) -> vela::Buffer {
    match vela::Buffer::load(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn kaelo_lang(path: &str) -> Option<&'static vela::Lang> {
    vela::detect(path)
}