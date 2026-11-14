# vela

The core of a text editor, in Rust, with no dependencies: buffer, cursor, find and
replace, syntax highlighting, file I/O. The host owns the window and the keyboard;
vela owns the text.

```rust
let mut b = Buffer::from_str("let x = 1;");
b.insert('!');
b.move_left();
b.newline();
assert_eq!(b.to_string(), "let x = 1!\n");
```

```
$ vela show main.rs
   1 | fn main() {
   2 |     let x = 42;
   3 |     // comment
   4 | }

$ vela stats main.rs
lines 4
chars 41
bytes 44

$ vela find "comment" main.rs
3:     // comment

$ vela replace "42" "7" main.rs
1 line(s) changed
```

## The editor

```bash
vela serve            # http://127.0.0.1:8420
```

One binary: it serves the UI (ui/dist) and a JSON API backed by the Rust core.
The browser never touches the filesystem — every read and write goes through the
core, so the same rules apply (UTF-8, and no path may escape `--root`).

```bash
vela serve --port 8420 --root C:/path/to/project
```

API: `GET /api/list?dir=`, `GET /api/file?path=`, `POST /api/save`,
`GET /api/highlight?path=&line=`.

The UI is React + Tailwind in `ui/`:

```bash
cd ui && npm install && npm run build   # writes ui/dist
```

## CLI

`vela.exe` is in the repository root. Run it from a terminal:

```
vela show <file>
```

Double-clicking it prints the help and waits, so nothing flashes closed. There is
nothing to install and no runtime to fetch — Node and Python are not involved.

## Build

```bash
cargo build --release
```

Rust 1.8x or newer. Nothing else — no crates, no runtime, no GC.

## What it does

- **Buffer** — lines, cursor, insert, newline, backspace, delete, arrow movement, clamped so the cursor can never leave the text
- **Find** — case-insensitive, from a given line
- **Replace** — case-insensitive, reports how many lines changed
- **Highlight** — keywords, strings (with escapes), line and block comments, numbers; rules per language, detected by extension
- **Languages** — Rust, Python, JavaScript/TypeScript
- **Files** — load and save, UTF-8

## What it is not

A complete editor. There is no window, no tabs, no settings, no plugin API, and no
Scintilla. Those live in the host application: vela is the text half, meant to be
embedded (C ABI on the roadmap, the same way kaelo exposes one).

## Tests

```bash
cargo test --release   # 12 tests: editing, cursor clamping, find, replace, file round-trip, highlighting
```

## License

MIT