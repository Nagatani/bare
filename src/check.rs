//! Checks the generated site for broken internal links, missing `#anchors`,
//! missing images and images without alt text.
//!
//! It reads the HTML that was actually written, so it covers links from
//! Markdown and templates alike, and resolves URLs the way GitHub Pages does.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug)]
pub struct Problem {
    pub severity: Severity,
    /// Page the problem was found in, relative to the output directory.
    pub page: PathBuf,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.page.display(), self.message)
    }
}

/// A start tag and its attributes, with entity references decoded.
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Checks every HTML page under `out_dir`. `base_path` is the URL path the site
/// is served under (e.g. `/bare/`), used for root-relative links.
pub fn check_site(out_dir: &Path, base_path: &str) -> Vec<Problem> {
    let mut pages = Vec::new();
    collect_html(out_dir, Path::new(""), &mut pages);
    pages.sort();

    let mut parsed: HashMap<PathBuf, Vec<Tag>> = HashMap::new();
    for page in &pages {
        if let Ok(html) = fs::read_to_string(out_dir.join(page)) {
            parsed.insert(page.clone(), parse_tags(&html));
        }
    }
    let ids: HashMap<&PathBuf, HashSet<&str>> = parsed
        .iter()
        .map(|(page, tags)| (page, tags.iter().filter_map(|t| t.attr("id")).collect()))
        .collect();

    let mut problems = Vec::new();
    for page in &pages {
        let Some(tags) = parsed.get(page) else {
            continue;
        };
        for tag in tags {
            if tag.name == "img" && tag.attr("alt").map_or(true, |alt| alt.trim().is_empty()) {
                problems.push(Problem {
                    severity: Severity::Warning,
                    page: page.clone(),
                    message: format!("image without alt text: {}", tag.attr("src").unwrap_or("?")),
                });
            }
            for attr in ["href", "src"] {
                let Some(url) = tag.attr(attr) else { continue };
                if let Some(message) = check_url(out_dir, base_path, page, url, &ids) {
                    problems.push(Problem {
                        severity: Severity::Error,
                        page: page.clone(),
                        message,
                    });
                }
            }
        }
    }
    problems
}

/// Returns a message if `url` (found in `page`) points at a missing file or anchor.
fn check_url(
    out_dir: &Path,
    base_path: &str,
    page: &Path,
    url: &str,
    ids: &HashMap<&PathBuf, HashSet<&str>>,
) -> Option<String> {
    if is_external(url) {
        return None;
    }
    let (path, fragment) = url.split_once('#').unwrap_or((url, ""));
    let path = path.split('?').next().unwrap_or("");
    let path = percent_decode(path);
    let fragment = percent_decode(fragment);

    let target = if path.is_empty() {
        page.to_path_buf()
    } else {
        let joined = match path.strip_prefix('/') {
            Some(_) => match path.strip_prefix(base_path) {
                Some(rest) => PathBuf::from(rest),
                None => return Some(format!("link outside the site ({base_path}): {url}")),
            },
            None => page.parent().unwrap_or(Path::new("")).join(&path),
        };
        let Some(normalized) = normalize(&joined) else {
            return Some(format!("link escapes the site root: {url}"));
        };
        match resolve_file(out_dir, &normalized, path.ends_with('/')) {
            Some(file) => file,
            None => return Some(format!("broken link: {url}")),
        }
    };

    let is_html = target.extension().is_some_and(|ext| ext == "html");
    if !fragment.is_empty() && is_html {
        let found = ids
            .get(&target)
            .is_some_and(|page_ids| page_ids.contains(fragment.as_str()));
        if !found {
            return Some(format!("missing anchor #{fragment}: {url}"));
        }
    }
    None
}

/// URLs with a scheme (`https:`, `mailto:`, `data:` …) or protocol-relative ones.
fn is_external(url: &str) -> bool {
    if url.starts_with("//") {
        return true;
    }
    match url.find(':') {
        Some(colon) => !url[..colon].contains(['/', '?', '#']),
        None => false,
    }
}

/// Resolves a site-relative path to an existing file like GitHub Pages:
/// directories serve `index.html`, and `a` may serve `a.html`.
fn resolve_file(out_dir: &Path, path: &Path, is_dir: bool) -> Option<PathBuf> {
    let full = out_dir.join(path);
    if is_dir || full.is_dir() {
        let index = path.join("index.html");
        return out_dir.join(&index).is_file().then_some(index);
    }
    if full.is_file() {
        return Some(path.to_path_buf());
    }
    let mut html = path.as_os_str().to_owned();
    html.push(".html");
    let html = PathBuf::from(html);
    out_dir.join(&html).is_file().then_some(html)
}

/// Resolves `.` and `..`; `None` if the path climbs above the site root.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(part) => out.push(part),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn collect_html(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root.join(rel)) else {
        return;
    };
    for entry in entries.flatten() {
        let rel = rel.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => collect_html(root, &rel, out),
            Ok(_) if rel.extension().is_some_and(|ext| ext == "html") => out.push(rel),
            _ => {}
        }
    }
}

/// Extracts start tags with their attributes. Text, comments and end tags are
/// skipped; escaped text such as `&lt;a href&gt;` is never mistaken for a tag.
fn parse_tags(html: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        let inner = &rest[..end];
        rest = rest.get(end + 1..).unwrap_or("");

        let name_len = inner
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(inner.len());
        let name = inner[..name_len].to_ascii_lowercase();
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
            continue;
        }
        tags.push(Tag {
            name,
            attrs: parse_attrs(&inner[name_len..]),
        });
    }
    tags
}

fn parse_attrs(mut s: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    loop {
        s = s.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if s.is_empty() {
            return attrs;
        }
        let name_len = s
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(s.len());
        let name = s[..name_len].to_ascii_lowercase();
        s = s[name_len..].trim_start();
        let mut value = String::new();
        if let Some(after) = s.strip_prefix('=') {
            let after = after.trim_start();
            let (raw, remaining) = match after.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let body = &after[1..];
                    let end = body.find(quote).unwrap_or(body.len());
                    (&body[..end], body.get(end + 1..).unwrap_or(""))
                }
                _ => {
                    let end = after.find(char::is_whitespace).unwrap_or(after.len());
                    (&after[..end], &after[end..])
                }
            };
            value = decode_entities(raw);
            s = remaining;
        }
        attrs.push((name, value));
    }
}

/// Decodes the entity references bare itself produces in attributes.
fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Decodes `%XX` escapes, leaving malformed sequences as they are.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let decoded = (bytes[i] == b'%')
            .then(|| s.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match decoded {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_external_urls() {
        assert!(is_external("https://example.com"));
        assert!(is_external("mailto:a@b.c"));
        assert!(is_external("//cdn.example/x.js"));
        assert!(!is_external("posts/a.html"));
        assert!(!is_external("../a.html#b:c"));
    }

    #[test]
    fn parses_tags_and_attributes() {
        let tags = parse_tags(
            "<!-- <a href=\"x\"> --><p>&lt;a href=&quot;y&quot;&gt;</p>\
             <a class=x href='a&amp;b.html'>t</a><img src=\"i.png\" alt>",
        );
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["p", "a", "img"]);
        assert_eq!(tags[1].attr("href"), Some("a&b.html"));
        assert_eq!(tags[1].attr("class"), Some("x"));
        assert_eq!(tags[2].attr("alt"), Some(""));
    }

    #[test]
    fn normalizes_paths() {
        assert_eq!(
            normalize(Path::new("posts/../index.html")),
            Some(PathBuf::from("index.html"))
        );
        assert_eq!(normalize(Path::new("../x")), None);
    }

    #[test]
    fn finds_broken_links_and_anchors() {
        let dir = std::env::temp_dir().join(format!("bare-check-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("posts")).unwrap();
        fs::write(
            dir.join("index.html"),
            "<a href=\"posts/a.html#ok\">a</a><a href=\"posts/b.html\">b</a>",
        )
        .unwrap();
        fs::write(
            dir.join("posts/a.html"),
            "<h2 id=\"ok\">x</h2><a href=\"../index.html\">home</a><a href=\"#nope\">n</a>\
             <a href=\"/bare/posts/a\">abs</a><a href=\"https://e.com\">e</a><img src=\"../i.png\">",
        )
        .unwrap();

        let problems = check_site(&dir, "/bare/");
        let messages: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
        assert_eq!(
            messages,
            [
                "index.html: broken link: posts/b.html",
                "posts/a.html: missing anchor #nope: #nope",
                "posts/a.html: image without alt text: ../i.png",
                "posts/a.html: broken link: ../i.png",
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
