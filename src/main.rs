use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return;
    }

    let cmd = args[0].as_str();
    let rest = &args[1..];

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
        "--version" | "-v" => println!("vela {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" => usage(),
        other => {
            println!("unknown command '{}'", other);
            usage();
        }
    }
    let _ = std::io::stdout().flush();
}

fn usage() {
    println!(
        "vela {} — a small text editor core\\n\\n  vela show <file>              numbered, syntax-highlighted view\\n  vela stats <file>             lines, chars, bytes\\n  vela find <pat> <file>        first matching line\\n  vela replace <pat> <with> <file>  replace in place, prints the count\\n  vela --version",
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