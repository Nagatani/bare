//! A tiny `{{placeholder}}` substitution engine.
//!
//! Templates are scanned once from left to right, so values inserted into the
//! output are never re-scanned: a post that contains a literal `{{title}}` stays
//! as written. Unknown placeholders are left untouched to make typos visible.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Values available to a template, keyed by placeholder name.
#[derive(Debug, Default, Clone)]
pub struct Context {
    values: HashMap<String, String>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.values.insert(key.to_string(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct Template {
    source: String,
}

impl Template {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        fs::read_to_string(path).map(Self::new)
    }

    /// True if the template contains the placeholder `{{name}}`.
    pub fn uses(&self, name: &str) -> bool {
        self.source.contains(&format!("{{{{{name}}}}}"))
    }

    /// Names of placeholders in the template that `context` does not provide.
    pub fn unknown_tokens(&self, context: &Context) -> Vec<String> {
        let mut unknown = Vec::new();
        let mut rest = self.source.as_str();
        while let Some(start) = rest.find("{{") {
            let inner = &rest[start + 2..];
            let Some(end) = inner.find("}}") else { break };
            let key = inner[..end].trim();
            if context.get(key).is_none() && !unknown.iter().any(|k| k == key) {
                unknown.push(key.to_string());
            }
            rest = &inner[end + 2..];
        }
        unknown
    }

    pub fn render(&self, context: &Context) -> String {
        let mut out = String::with_capacity(self.source.len());
        let mut rest = self.source.as_str();

        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let inner = &rest[start + 2..];
            let Some(end) = inner.find("}}") else {
                // Unclosed `{{`: emit the remainder verbatim.
                out.push_str(&rest[start..]);
                return out;
            };
            match context.get(inner[..end].trim()) {
                Some(value) => out.push_str(value),
                None => out.push_str(&rest[start..start + 2 + end + 2]),
            }
            rest = &inner[end + 2..];
        }

        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_known_tokens() {
        let tpl = Template::new("<h1>{{ title }}</h1>{{content}}");
        let ctx = Context::new()
            .with("title", "Hi")
            .with("content", "<p>{{title}}</p>");
        assert_eq!(tpl.render(&ctx), "<h1>Hi</h1><p>{{title}}</p>");
    }

    #[test]
    fn lists_unknown_tokens() {
        let tpl = Template::new("{{a}} {{ b }} {{a}} {{c}}");
        let ctx = Context::new().with("c", "");
        assert_eq!(tpl.unknown_tokens(&ctx), ["a", "b"]);
    }

    #[test]
    fn keeps_unknown_and_unclosed_tokens() {
        let tpl = Template::new("{{nope}} and {{open");
        assert_eq!(tpl.render(&Context::new()), "{{nope}} and {{open");
    }
}
