//! vela — a small text editor core.
//!
//! The buffer, cursor, find and syntax highlighting a GUI editor needs, with no
//! dependencies: the host owns the window and the keyboard, vela owns the text.

pub mod serve;

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub fn new(line: usize, col: usize) -> Self {
        Pos { line, col }
    }
}

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error(msg.into()))
}

/// The text: a vector of lines, plus a cursor. Cheap to clone, no surprises.
#[derive(Debug, Clone)]
pub struct Buffer {
    pub lines: Vec<String>,
    pub cursor: Pos,
}

impl Buffer {
    pub fn new() -> Self {
        Buffer { lines: vec![String::new()], cursor: Pos::new(0, 0) }
    }

    pub fn from_str(text: &str) -> Self {
        let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Buffer { lines, cursor: Pos::new(0, 0) }
    }

    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error(format!("{}: {}", path, e)))?;
        Ok(Buffer::from_str(&text))
    }

    pub fn save(&self, path: &str) -> Result<()> {
        std::fs::write(path, self.to_string()).map_err(|e| Error(format!("{}: {}", path, e)))
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, i: usize) -> &str {
        self.lines.get(i).map(|s| s.as_str()).unwrap_or("")
    }

    fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor.line >= self.lines.len() {
            self.cursor.line = self.lines.len() - 1;
        }
        let len = self.lines[self.cursor.line].chars().count();
        if self.cursor.col > len {
            self.cursor.col = len;
        }
    }

    pub fn insert(&mut self, ch: char) {
        let (l, c) = (self.cursor.line, self.cursor.col);
        let line = &mut self.lines[l];
        let byte = char_to_byte(line, c);
        line.insert(byte, ch);
        self.cursor.col += 1;
    }

    pub fn newline(&mut self) {
        let (l, c) = (self.cursor.line, self.cursor.col);
        let line = &mut self.lines[l];
        let byte = char_to_byte(line, c);
        let rest = line.split_off(byte);
        self.lines.insert(l + 1, rest);
        self.cursor.line += 1;
        self.cursor.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor.col == 0 {
            if self.cursor.line == 0 {
                return;
            }
            let l = self.cursor.line;
            let cur = self.lines.remove(l);
            self.cursor.line -= 1;
            self.cursor.col = self.lines[self.cursor.line].chars().count();
            self.lines[self.cursor.line].push_str(&cur);
            return;
        }
        let (l, c) = (self.cursor.line, self.cursor.col);
        let line = &mut self.lines[l];
        let byte = char_to_byte(line, c - 1);
        let prev = line[..byte].chars().next_back();
        if let Some(p) = prev {
            let cut = byte - p.len_utf8();
            line.replace_range(cut..byte, "");
        }
        self.cursor.col -= 1;
    }

    pub fn delete(&mut self) {
        let (l, c) = (self.cursor.line, self.cursor.col);
        if c >= self.lines[l].chars().count() {
            if l + 1 >= self.lines.len() {
                return;
            }
            let next = self.lines.remove(l + 1);
            self.lines[l].push_str(&next);
            return;
        }
        let line = &mut self.lines[l];
        let byte = char_to_byte(line, c);
        let ch = line[byte..].chars().next().unwrap();
        line.replace_range(byte..byte + ch.len_utf8(), "");
    }

    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.lines[self.cursor.line].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let len = self.lines[self.cursor.line].chars().count();
        if self.cursor.col < len {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.clamp_cursor();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.clamp_cursor();
        }
    }

    /// First line containing `needle`, at or after `from`. Case-insensitive.
    pub fn find(&self, needle: &str, from: usize) -> Option<usize> {
        let n = needle.to_lowercase();
        (from..self.lines.len()).find(|&i| self.lines[i].to_lowercase().contains(&n))
    }

    pub fn replace(&mut self, needle: &str, with: &str) -> usize {
        let mut count = 0;
        for line in self.lines.iter_mut() {
            if line.to_lowercase().contains(&needle.to_lowercase()) {
                *line = replace_ci(line, needle, with);
                count += 1;
            }
        }
        count
    }

    pub fn stats(&self) -> Stats {
        let chars: usize = self.lines.iter().map(|l| l.chars().count()).sum();
        Stats {
            lines: self.lines.len(),
            chars,
            bytes: self.to_string().len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub lines: usize,
    pub chars: usize,
    pub bytes: usize,
}

fn char_to_byte(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map(|(b, _)| b).unwrap_or(s.len())
}

fn replace_ci(hay: &str, needle: &str, with: &str) -> String {
    let mut out = String::new();
    let mut rest = hay;
    loop {
        let lower = rest.to_lowercase();
        match lower.find(&needle.to_lowercase()) {
            Some(byte) => {
                out.push_str(&rest[..byte]);
                out.push_str(with);
                let skip = needle.len().min(rest.len() - byte);
                rest = &rest[byte + skip..];
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

/// A language's syntax rules: keywords, line comment, and whether it has strings.
#[derive(Debug, Clone, Copy)]
pub struct Lang {
    pub name: &'static str,
    pub keywords: &'static [&'static str],
    pub line_comment: &'static str,
    pub block_comment: Option<(&'static str, &'static str)>,
    pub extensions: &'static [&'static str],
}

pub const LANGS: &[Lang] = &[
    Lang {
        name: "rust",
        keywords: &["fn", "let", "mut", "struct", "enum", "impl", "if", "else", "while",
                    "for", "loop", "match", "return", "pub", "use", "mod", "crate", "self"],
        line_comment: "//",
        block_comment: Some(("/*", "*/")),
        extensions: &["rs"],
    },
    Lang {
        name: "python",
        keywords: &["def", "class", "if", "elif", "else", "while", "for", "in", "return",
                    "import", "from", "as", "try", "except", "finally", "with", "lambda", "pass"],
        line_comment: "#",
        block_comment: None,
        extensions: &["py"],
    },
    Lang {
        name: "javascript",
        keywords: &["function", "let", "const", "var", "if", "else", "while", "for", "return",
                    "class", "new", "import", "export", "async", "await", "typeof"],
        line_comment: "//",
        block_comment: Some(("/*", "*/")),
        extensions: &["js", "ts", "jsx", "tsx"],
    },
];

pub fn detect(path: &str) -> Option<&'static Lang> {
    let ext = path.rsplit('.').next().unwrap_or("");
    LANGS.iter().find(|l| l.extensions.contains(&ext))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Keyword,
    String,
    Comment,
    Number,
    Plain,
}

/// Split a line into (kind, text) spans. Single-line: block comments are
/// reported per line, which is what a highlighter needs.
pub fn highlight(lang: &Lang, line: &str) -> Vec<(Kind, String)> {
    let mut out: Vec<(Kind, String)> = Vec::new();
    let bytes = line.as_bytes();

    let mut i = 0usize;
    let mut plain = String::new();
    while i < bytes.len() {
        let rest = &line[i..];

        if rest.starts_with(lang.line_comment) {
            if !plain.is_empty() {
                out.push((Kind::Plain, std::mem::take(&mut plain)));
            }
            out.push((Kind::Comment, rest.to_string()));
            return out;
        }
        if let Some((open, close)) = lang.block_comment {
            if rest.starts_with(open) {
                if !plain.is_empty() {
                    out.push((Kind::Plain, std::mem::take(&mut plain)));
                }
                match rest.find(close) {
                    Some(end) => {
                        let cut = end + close.len();
                        out.push((Kind::Comment, rest[..cut].to_string()));
                        i += cut;
                        continue;
                    }
                    None => {
                        out.push((Kind::Comment, rest.to_string()));
                        return out;
                    }
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        if ch == '"' || ch == '\'' {
            if !plain.is_empty() {
                out.push((Kind::Plain, std::mem::take(&mut plain)));
            }
            let quote = ch;
            let mut s = String::new();
            s.push(quote);
            let mut it = rest[quote.len_utf8()..].chars();
            while let Some(c) = it.next() {
                s.push(c);
                if c == '\\' {
                    if let Some(n) = it.next() {
                        s.push(n);
                    }
                    continue;
                }
                if c == quote {
                    break;
                }
            }
            let used = s.len();
            out.push((Kind::String, s));
            i += used;
            continue;
        }
        if ch.is_ascii_digit() {
            if !plain.is_empty() {
                out.push((Kind::Plain, std::mem::take(&mut plain)));
            }
            let mut n = String::new();
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                n.push(bytes[j] as char);
                j += 1;
            }
            out.push((Kind::Number, n));
            i = j;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &line[i..j];
            if lang.keywords.contains(&word) {
                if !plain.is_empty() {
                    out.push((Kind::Plain, std::mem::take(&mut plain)));
                }
                out.push((Kind::Keyword, word.to_string()));
            } else {
                plain.push_str(word);
            }
            i = j;
            continue;
        }
        plain.push(ch);
        i += ch.len_utf8();
    }
    if !plain.is_empty() {
        out.push((Kind::Plain, plain));
    }
    out
}

/// Keywords across all languages, for tests and for a simple completion source.
pub fn all_keywords() -> HashSet<&'static str> {
    LANGS.iter().flat_map(|l| l.keywords.iter().copied()).collect()
}