//! Build-time CSS bundling: `@import "_part.css";` is replaced with the file's
//! contents, so the source can be split into small files while browsers still
//! download a single stylesheet (no chain of requests).
//!
//! Only plain imports of local files are inlined. Imports with media queries,
//! `layer()` or `supports()`, and imports of URLs, are left for the browser.
//! Files whose name starts with `_` and ends in `.css` are partials: they are
//! inlined where imported and not copied to the output on their own.
//!
//! Comments and blank lines are removed from the output, so the source can be
//! documented freely without making pages heavier.

use std::fs;
use std::path::{Path, PathBuf};

/// True for partials such as `_tokens.css`, which are not copied on their own.
pub fn is_partial(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with('_') && name.ends_with(".css")
}

/// Reads the stylesheet at `path` with its local `@import`s inlined. Problems
/// (missing files, import cycles) are added to `warnings` and leave the
/// `@import` line as it was.
pub fn bundle(path: &Path, warnings: &mut Vec<String>) -> std::io::Result<String> {
    let text = fs::read_to_string(path)?;
    let mut stack = vec![canonical(path)];
    let css = inline(&text, path, &mut stack, warnings);
    let mut out = String::with_capacity(css.len());
    for line in css
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
    {
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn inline(text: &str, path: &Path, stack: &mut Vec<PathBuf>, warnings: &mut Vec<String>) -> String {
    let dir = path.parent().unwrap_or(Path::new(""));
    // Comments go first, so a commented-out `@import` is not inlined.
    let text = strip_comments(text);
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let Some(target) = import_target(line.trim()) else {
            out.push_str(line);
            continue;
        };
        let file = dir.join(target);
        let key = canonical(&file);
        if stack.contains(&key) {
            warnings.push(format!("{}: @import cycle at {target}", path.display()));
            out.push_str(line);
            continue;
        }
        match fs::read_to_string(&file) {
            Ok(imported) => {
                stack.push(key);
                let body = inline(&imported, &file, stack, warnings);
                stack.pop();
                out.push_str(&body);
                if !body.ends_with('\n') {
                    out.push('\n');
                }
            }
            Err(err) => {
                warnings.push(format!("{}: cannot import {target}: {err}", path.display()));
                out.push_str(line);
            }
        }
    }
    out
}

/// Removes `/* … */` comments, leaving quoted strings such as `content: "/*"` alone.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    let mut quote: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    // Keep an escaped character, whatever it is.
                    let escaped = rest[1..].chars().next().map_or(0, char::len_utf8);
                    out.push_str(&rest[..1 + escaped]);
                    rest = &rest[1 + escaped..];
                    continue;
                }
                if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if rest.starts_with("/*") => {
                rest = rest[2..].find("*/").map_or("", |end| &rest[2 + end + 2..]);
                continue;
            }
            None => {}
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The file of `@import "x.css";` or `@import url("x.css");` when it is a plain
/// import of a local file.
fn import_target(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("@import")?.trim_start();
    let rest = rest.strip_suffix(';')?.trim_end();
    let rest = match rest.strip_prefix("url(") {
        Some(inner) => inner.strip_suffix(')')?.trim(),
        None => rest,
    };
    let unquoted = ['"', '\'']
        .into_iter()
        .find_map(|q| rest.strip_prefix(q)?.strip_suffix(q))
        .unwrap_or(rest);
    // Anything after the URL (media, layer(), supports()) leaves quotes or spaces behind.
    let is_local = !unquoted.is_empty()
        && !unquoted.contains(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | ')'))
        && !unquoted.contains(':')
        && !unquoted.starts_with('/');
    is_local.then_some(unquoted)
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_plain_local_imports() {
        assert_eq!(import_target("@import \"_a.css\";"), Some("_a.css"));
        assert_eq!(
            import_target("@import url('parts/_b.css');"),
            Some("parts/_b.css")
        );
        assert_eq!(import_target("@import url(_c.css);"), Some("_c.css"));
        assert_eq!(import_target("@import \"a.css\" layer(x);"), None);
        assert_eq!(import_target("@import \"a.css\" print;"), None);
        assert_eq!(import_target("@import url(https://x.test/a.css);"), None);
        assert_eq!(import_target("@import \"/abs.css\";"), None);
        assert_eq!(import_target("@layer a, b;"), None);
        assert!(is_partial(Path::new("css/_tokens.css")));
        assert!(!is_partial(Path::new("css/style.css")));
        assert!(!is_partial(Path::new("_headers")));
    }

    #[test]
    fn strips_comments_outside_strings() {
        assert_eq!(
            strip_comments("a { /* x */ content: \"/* kept */\"; } /* multi\nline */b{}"),
            "a {  content: \"/* kept */\"; } b{}"
        );
        assert_eq!(
            strip_comments("a { content: '\\'/*'; }"),
            "a { content: '\\'/*'; }"
        );
        assert_eq!(strip_comments("a {} /* unterminated"), "a {} ");
    }

    #[test]
    fn inlines_imports_recursively_and_stops_cycles() {
        let dir = std::env::temp_dir().join(format!("bare-css-{}", std::process::id()));
        fs::create_dir_all(dir.join("parts")).unwrap();
        fs::write(
            dir.join("style.css"),
            "/* entry */\n@layer a;\n@import \"parts/_a.css\";\n/*\n@import \"_b.css\";\n*/\nend {}\n",
        )
        .unwrap();
        fs::write(dir.join("parts/_a.css"), "a {}\n@import \"_b.css\";").unwrap();
        fs::write(
            dir.join("parts/_b.css"),
            "b {}\n@import \"_a.css\";\n@import \"_missing.css\";\n",
        )
        .unwrap();

        let mut warnings = Vec::new();
        let css = bundle(&dir.join("style.css"), &mut warnings).unwrap();
        assert_eq!(
            css,
            "@layer a;\na {}\nb {}\n@import \"_a.css\";\n@import \"_missing.css\";\nend {}\n"
        );
        assert_eq!(warnings.len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
