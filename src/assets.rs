//! Cache busting: local stylesheets and scripts referenced from a page get a
//! `?v=<hash>` query, so browsers and CDNs fetch them again only when they change.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Adds content hashes to asset URLs, remembering each file's hash.
pub struct Fingerprints<'a> {
    out_dir: &'a Path,
    /// URL path the site is served under (e.g. `/bare/`), for root-relative URLs.
    base_path: &'a str,
    cache: HashMap<PathBuf, Option<String>>,
}

impl<'a> Fingerprints<'a> {
    pub fn new(out_dir: &'a Path, base_path: &'a str) -> Self {
        Self {
            out_dir,
            base_path,
            cache: HashMap::new(),
        }
    }

    /// Rewrites `href`/`src` of `<link>` and `<script>` tags in a page written
    /// to `page_dir` that point to a local `.css` or `.js` file.
    pub fn apply(&mut self, html: &str, page_dir: &Path) -> String {
        let mut out = String::with_capacity(html.len() + 64);
        let mut rest = html;
        while let Some(start) = find_tag(rest) {
            let end = rest[start..]
                .find('>')
                .map_or(rest.len(), |i| start + i + 1);
            out.push_str(&rest[..start]);
            out.push_str(&self.rewrite_tag(&rest[start..end], page_dir));
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    }

    fn rewrite_tag(&mut self, tag: &str, page_dir: &Path) -> String {
        for attr in [" href=\"", " src=\""] {
            let Some(at) = tag.find(attr).map(|i| i + attr.len()) else {
                continue;
            };
            let Some(len) = tag[at..].find('"') else {
                continue;
            };
            let url = &tag[at..at + len];
            if let Some(hash) = self.file(url, page_dir).and_then(|file| self.hash(file)) {
                return format!("{}{url}?v={hash}{}", &tag[..at], &tag[at + len..]);
            }
        }
        tag.to_string()
    }

    /// The file a local asset URL points to, or `None` for other URLs.
    fn file(&self, url: &str, page_dir: &Path) -> Option<PathBuf> {
        let is_asset = url.ends_with(".css") || url.ends_with(".js");
        if !is_asset || url.contains([':', '?', '#']) || url.starts_with("//") {
            return None;
        }
        let file = match url.strip_prefix('/') {
            Some(_) => self.out_dir.join(url.strip_prefix(self.base_path)?),
            None => page_dir.join(url),
        };
        Some(file)
    }

    fn hash(&mut self, file: PathBuf) -> Option<String> {
        self.cache
            .entry(file)
            .or_insert_with_key(|file| fs::read(file).ok().map(|data| fnv1a(&data)))
            .clone()
    }
}

/// Start of the next `<link` or `<script` tag.
fn find_tag(html: &str) -> Option<usize> {
    let link = html.find("<link ");
    let script = html.find("<script ");
    match (link, script) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// First 8 hex digits of the 64-bit FNV-1a hash.
fn fnv1a(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", hash)[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_local_assets() {
        let dir = std::env::temp_dir().join(format!("bare-assets-{}", std::process::id()));
        fs::create_dir_all(dir.join("css")).unwrap();
        fs::write(dir.join("css/a.css"), "body{}").unwrap();
        fs::create_dir_all(dir.join("posts")).unwrap();
        let mut fp = Fingerprints::new(&dir, "/bare/");
        let hash = fnv1a(b"body{}");

        let html = "<link rel=\"stylesheet\" href=\"../css/a.css\">\
                    <link rel=\"icon\" href=\"../favicon.svg\">\
                    <link rel=\"stylesheet\" href=\"https://x.test/a.css\">\
                    <link rel=\"stylesheet\" href=\"../css/missing.css\">";
        assert_eq!(
            fp.apply(html, &dir.join("posts")),
            format!(
                "<link rel=\"stylesheet\" href=\"../css/a.css?v={hash}\">\
                 <link rel=\"icon\" href=\"../favicon.svg\">\
                 <link rel=\"stylesheet\" href=\"https://x.test/a.css\">\
                 <link rel=\"stylesheet\" href=\"../css/missing.css\">"
            )
        );
        assert_eq!(
            fp.apply("<link href=\"/bare/css/a.css\">", &dir),
            format!("<link href=\"/bare/css/a.css?v={hash}\">")
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
