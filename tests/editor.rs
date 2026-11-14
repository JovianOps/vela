use vela::*;

#[test]
fn insert_and_cursor() {
    let mut b = Buffer::new();
    b.insert('h');
    b.insert('i');
    assert_eq!(b.to_string(), "hi");
    assert_eq!(b.cursor, Pos::new(0, 2));
    b.move_left();
    b.insert('X');
    assert_eq!(b.to_string(), "hXi");
}

#[test]
fn newline_splits_lines() {
    let mut b = Buffer::from_str("ab");
    b.cursor = Pos::new(0, 1);
    b.newline();
    assert_eq!(b.to_string(), "a\nb");
    assert_eq!(b.cursor, Pos::new(1, 0));
}

#[test]
fn backspace_joins_lines() {
    let mut b = Buffer::from_str("ab\ncd");
    b.cursor = Pos::new(1, 0);
    b.backspace();
    assert_eq!(b.to_string(), "abcd");
    assert_eq!(b.cursor, Pos::new(0, 2));
}

#[test]
fn delete_removes_forward() {
    let mut b = Buffer::from_str("abc");
    b.delete();
    assert_eq!(b.to_string(), "bc");
}

#[test]
fn cursor_never_leaves_the_buffer() {
    let mut b = Buffer::from_str("x");
    for _ in 0..10 {
        b.move_up();
        b.move_left();
    }
    assert_eq!(b.cursor, Pos::new(0, 0));
    for _ in 0..10 {
        b.move_down();
        b.move_right();
    }
    assert_eq!(b.cursor, Pos::new(0, 1));
}

#[test]
fn find_is_case_insensitive() {
    let b = Buffer::from_str("alpha\nbeta\nGamma");
    assert_eq!(b.find("gamma", 0), Some(2));
    assert_eq!(b.find("beta", 0), Some(1));
    assert_eq!(b.find("nope", 0), None);
    assert_eq!(b.find("a", 3), None);
}

#[test]
fn replace_counts_lines() {
    let mut b = Buffer::from_str("foo\nFOO\nbar");
    let n = b.replace("foo", "baz");
    assert_eq!(n, 2);
    assert_eq!(b.to_string(), "baz\nbaz\nbar");
}

#[test]
fn round_trip_through_a_file() {
    let dir = std::env::temp_dir().join("vela-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.txt");
    let path = path.to_str().unwrap();

    let mut b = Buffer::from_str("line one\nline two");
    b.cursor = Pos::new(0, 8); // end of the first line
    b.insert('!');
    b.save(path).unwrap();

    let back = Buffer::load(path).unwrap();
    assert_eq!(back.line_count(), 2);
    assert_eq!(back.line(0), "line one!");
    let _ = std::fs::remove_file(path);
}

#[test]
fn highlights_keywords_strings_comments() {
    let lang = detect("main.rs").expect("rust detected");
    let spans = highlight(lang, "let s = \"hi\"; // note");
    let kinds: Vec<Kind> = spans.iter().map(|(k, _)| *k).collect();
    assert!(kinds.contains(&Kind::Keyword), "keywords: {:?}", spans);
    assert!(kinds.contains(&Kind::String), "strings: {:?}", spans);
    assert!(kinds.contains(&Kind::Comment), "comments: {:?}", spans);
    assert!(kinds.contains(&Kind::Number) == false, "no numbers here: {:?}", spans);
}

#[test]
fn highlights_numbers() {
    let lang = detect("a.py").expect("python detected");
    let spans = highlight(lang, "x = 42");
    assert!(spans.iter().any(|(k, t)| *k == Kind::Number && t == "42"), "spans: {:?}", spans);
}

#[test]
fn detects_language_by_extension() {
    assert_eq!(detect("a.rs").unwrap().name, "rust");
    assert_eq!(detect("a.py").unwrap().name, "python");
    assert_eq!(detect("a.js").unwrap().name, "javascript");
    assert!(detect("a.txt").is_none());
}

#[test]
fn python_uses_hash_comments() {
    let lang = detect("a.py").unwrap();
    let spans = highlight(lang, "# a comment");
    assert!(spans.iter().all(|(k, _)| *k == Kind::Comment), "spans: {:?}", spans);
}