//! Plain-text helpers over rendered HTML: excerpts and reading time.

/// Characters kept in an automatic excerpt.
const EXCERPT_CHARS: usize = 120;
/// Reading speed for Japanese/Chinese text, in characters per minute.
const WIDE_CHARS_PER_MINUTE: usize = 500;
/// Reading speed for space-separated text, in words per minute.
const WORDS_PER_MINUTE: usize = 200;

/// Text of the first paragraph, shortened to about 120 characters. Used as the
/// description when a post does not set one.
pub fn excerpt(html: &str) -> String {
    let Some(start) = html.find("<p>") else {
        return String::new();
    };
    let rest = &html[start + 3..];
    let paragraph = &rest[..rest.find("</p>").unwrap_or(rest.len())];
    let text = plain_text(paragraph);
    let text = text.trim();
    if text.chars().count() <= EXCERPT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(EXCERPT_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// Estimated minutes to read `html`, not counting code blocks; at least 1.
pub fn reading_minutes(html: &str) -> usize {
    let mut prose = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        prose.push_str(&rest[..start]);
        rest = match rest[start..].find("</pre>") {
            Some(end) => &rest[start + end + 6..],
            None => "",
        };
    }
    prose.push_str(rest);

    let text = plain_text(&prose);
    let wide = text.chars().filter(|&c| c >= '\u{2E80}').count();
    let words = text
        .split(|c: char| c.is_whitespace() || c >= '\u{2E80}')
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count();
    let minutes =
        wide as f64 / WIDE_CHARS_PER_MINUTE as f64 + words as f64 / WORDS_PER_MINUTE as f64;
    (minutes.ceil() as usize).max(1)
}

/// Removes tags, decodes the entities `escape_html` produces and collapses whitespace.
fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            rest = &rest[rest.find('>').map_or(rest.len(), |i| i + 1)..];
            continue;
        }
        if c == '&' {
            let decoded = [
                ("&amp;", '&'),
                ("&lt;", '<'),
                ("&gt;", '>'),
                ("&quot;", '"'),
                ("&#39;", '\''),
            ]
            .into_iter()
            .find(|(entity, _)| rest.starts_with(entity));
            if let Some((entity, ch)) = decoded {
                out.push(ch);
                rest = &rest[entity.len()..];
                continue;
            }
        }
        if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_takes_first_paragraph() {
        let html = "<h2 id=\"x\">Title</h2>\n<p>Hello <a href=\"#\">world</a> &amp; <code>&lt;b&gt;</code>.</p>\n<p>Next</p>";
        assert_eq!(excerpt(html), "Hello world & <b>.");
        assert_eq!(excerpt("<ul><li>x</li></ul>"), "");
        let long = format!("<p>{}</p>", "あ".repeat(200));
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), EXCERPT_CHARS + 1);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn estimates_reading_time() {
        assert_eq!(reading_minutes("<p>short</p>"), 1);
        let japanese = format!("<p>{}</p>", "あ".repeat(1200));
        assert_eq!(reading_minutes(&japanese), 3);
        let english = format!("<p>{}</p>", "word ".repeat(450));
        assert_eq!(reading_minutes(&english), 3);
        let code = format!("<p>x</p><pre><code>{}</code></pre>", "code ".repeat(5000));
        assert_eq!(reading_minutes(&code), 1);
    }
}
