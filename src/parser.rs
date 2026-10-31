//! Frontmatter and minimal Markdown parsing.
//!
//! Everything here is plain string slicing over lines — no regex, no external
//! Markdown crate. The supported subset is intentionally small:
//! headings, fenced code blocks, inline code, unordered lists, paragraphs and links.

use std::collections::HashMap;

/// Key-value metadata read from the `---` block at the top of a document.
#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    fields: HashMap<String, String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// Parses `tags` given either as `rust, ssg` or `[rust, ssg]`.
    pub fn tags(&self) -> Vec<String> {
        let Some(raw) = self.get("tags") else {
            return Vec::new();
        };
        let raw = raw.trim();
        let raw = raw.strip_prefix('[').unwrap_or(raw);
        let raw = raw.strip_suffix(']').unwrap_or(raw);
        raw.split(',')
            .map(|tag| unquote(tag.trim()).to_string())
            .filter(|tag| !tag.is_empty())
            .collect()
    }
}

/// A source file split into its frontmatter and Markdown body.
#[derive(Debug)]
pub struct Document {
    pub frontmatter: Frontmatter,
    pub body: String,
}

/// Splits `source` into frontmatter and body.
///
/// The frontmatter must start on the very first line with `---` and end with
/// another `---` line. Without a closing delimiter the whole input is treated as body.
pub fn parse_document(source: &str) -> Document {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let whole = || Document {
        frontmatter: Frontmatter::default(),
        body: source.to_string(),
    };

    let mut lines = source.lines();
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return whole(),
    }

    let mut fields = HashMap::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        insert_field(&mut fields, line);
    }
    if !closed {
        return whole();
    }

    Document {
        frontmatter: Frontmatter { fields },
        body: lines.collect::<Vec<_>>().join("\n"),
    }
}

/// Parses a whole file of `key: value` lines, such as `site.conf`.
pub fn parse_key_values(text: &str) -> Frontmatter {
    let mut fields = HashMap::new();
    for line in text.lines() {
        insert_field(&mut fields, line);
    }
    Frontmatter { fields }
}

/// Inserts one `key: value` line; lines without a colon or starting with `#` are ignored.
fn insert_field(fields: &mut HashMap<String, String>, line: &str) {
    if let Some((key, value)) = line.split_once(':') {
        let key = key.trim();
        if !key.is_empty() && !key.starts_with('#') {
            fields.insert(key.to_string(), unquote(value.trim()).to_string());
        }
    }
}

/// Removes one pair of matching surrounding quotes (`"..."` or `'...'`).
fn unquote(s: &str) -> &str {
    for quote in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(quote) && s.ends_with(quote) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Converts the supported Markdown subset to HTML.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() * 2);
    let mut paragraph: Vec<&str> = Vec::new();
    let mut list: Vec<&str> = Vec::new();
    // Language tag and raw lines of the fenced code block currently open, if any.
    let mut code: Option<(&str, Vec<&str>)> = None;

    for line in markdown.lines() {
        if code.is_some() {
            if line.trim_start().starts_with("```") {
                if let Some((lang, lines)) = code.take() {
                    push_code_block(&mut out, lang, &lines);
                }
            } else if let Some((_, lines)) = code.as_mut() {
                // Keep the line untouched: indentation matters inside code.
                lines.push(line);
            }
            continue;
        }

        let trimmed = line.trim();

        if let Some(lang) = trimmed.strip_prefix("```") {
            flush_paragraph(&mut out, &mut paragraph);
            flush_list(&mut out, &mut list);
            code = Some((lang.trim(), Vec::new()));
        } else if trimmed.is_empty() {
            flush_paragraph(&mut out, &mut paragraph);
            flush_list(&mut out, &mut list);
        } else if let Some((level, text)) = parse_heading(trimmed) {
            flush_paragraph(&mut out, &mut paragraph);
            flush_list(&mut out, &mut list);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", render_inline(text)));
        } else if let Some(item) = trimmed.strip_prefix("- ") {
            flush_paragraph(&mut out, &mut paragraph);
            list.push(item.trim());
        } else {
            flush_list(&mut out, &mut list);
            paragraph.push(trimmed);
        }
    }

    // An unterminated fence still renders everything after it as code.
    if let Some((lang, lines)) = code.take() {
        push_code_block(&mut out, lang, &lines);
    }
    flush_paragraph(&mut out, &mut paragraph);
    flush_list(&mut out, &mut list);
    out
}

/// Recognises `# ` through `###### ` and returns the level and heading text.
fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let text = line[level..].strip_prefix(' ')?;
    Some((level, text.trim()))
}

fn flush_paragraph(out: &mut String, paragraph: &mut Vec<&str>) {
    if paragraph.is_empty() {
        return;
    }
    out.push_str("<p>");
    out.push_str(&render_inline(&paragraph.join(" ")));
    out.push_str("</p>\n");
    paragraph.clear();
}

fn flush_list(out: &mut String, list: &mut Vec<&str>) {
    if list.is_empty() {
        return;
    }
    out.push_str("<ul>\n");
    for item in list.iter() {
        out.push_str("  <li>");
        out.push_str(&render_inline(item));
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n");
    list.clear();
}

fn push_code_block(out: &mut String, lang: &str, lines: &[&str]) {
    if lang.is_empty() {
        out.push_str("<pre><code>");
    } else {
        out.push_str(&format!(
            "<pre><code class=\"language-{}\">",
            escape_html(lang)
        ));
    }
    out.push_str(&escape_html(&lines.join("\n")));
    out.push_str("</code></pre>\n");
}

/// Renders inline code and links; everything else is HTML-escaped text.
fn render_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(c) = rest.chars().next() {
        match c {
            '`' => {
                if let Some(end) = rest[1..].find('`') {
                    out.push_str("<code>");
                    out.push_str(&escape_html(&rest[1..1 + end]));
                    out.push_str("</code>");
                    rest = &rest[end + 2..];
                    continue;
                }
            }
            '[' => {
                if let Some((label, url, consumed)) = parse_link(rest) {
                    out.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        escape_html(url),
                        render_inline(label)
                    ));
                    rest = &rest[consumed..];
                    continue;
                }
            }
            _ => {}
        }
        push_escaped(&mut out, c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Parses `[label](url)` at the start of `s`, returning label, url and bytes consumed.
fn parse_link(s: &str) -> Option<(&str, &str, usize)> {
    let close = s.find(']')?;
    let after = s[close + 1..].strip_prefix('(')?;
    let url_end = after.find(')')?;
    let label = &s[1..close];
    let url = after[..url_end].trim();
    Some((label, url, close + 2 + url_end + 1))
}

/// Escapes the characters that are significant in HTML text and attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        push_escaped(&mut out, c);
    }
    out
}

fn push_escaped(out: &mut String, c: char) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let doc = parse_document("---\ntitle: \"Hi: there\"\ndate: 2026-01-02\ntags: [a, b]\n---\nBody");
        assert_eq!(doc.frontmatter.get("title"), Some("Hi: there"));
        assert_eq!(doc.frontmatter.get("date"), Some("2026-01-02"));
        assert_eq!(doc.frontmatter.tags(), vec!["a", "b"]);
        assert_eq!(doc.body, "Body");
    }

    #[test]
    fn parses_key_value_file() {
        let conf = parse_key_values("# comment: ignored\ntitle: My Blog\n\nurl: https://example.com\n");
        assert_eq!(conf.get("title"), Some("My Blog"));
        assert_eq!(conf.get("url"), Some("https://example.com"));
        assert!(conf.get("# comment").is_none());
    }

    #[test]
    fn missing_frontmatter_keeps_body() {
        let doc = parse_document("# Title\n---\n");
        assert!(doc.frontmatter.get("title").is_none());
        assert_eq!(doc.body, "# Title\n---\n");
    }

    #[test]
    fn renders_blocks() {
        let md = "# One\n## Two\n### Three\n\nline a\nline b\n\n- x\n- `y`\n";
        assert_eq!(
            markdown_to_html(md),
            "<h1>One</h1>\n<h2>Two</h2>\n<h3>Three</h3>\n<p>line a line b</p>\n\
             <ul>\n  <li>x</li>\n  <li><code>y</code></li>\n</ul>\n"
        );
    }

    #[test]
    fn escapes_code_blocks() {
        let md = "```rust\nlet v: Vec<u8> = vec![];\n  {{title}}\n```";
        assert_eq!(
            markdown_to_html(md),
            "<pre><code class=\"language-rust\">let v: Vec&lt;u8&gt; = vec![];\n  {{title}}</code></pre>\n"
        );
    }

    #[test]
    fn renders_inline() {
        assert_eq!(
            render_inline("see [the `std` docs](https://doc.rust-lang.org/std/) & <b>"),
            "see <a href=\"https://doc.rust-lang.org/std/\">the <code>std</code> docs</a> &amp; &lt;b&gt;"
        );
        assert_eq!(render_inline("[not a link] `open"), "[not a link] `open");
    }
}
