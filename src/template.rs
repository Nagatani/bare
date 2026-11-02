//! A tiny `{{placeholder}}` substitution engine.
//!
//! Templates are scanned once from left to right, so values inserted into the
//! output are never re-scanned: a post that contains a literal `{{title}}` stays
//! as written. Unknown placeholders are left untouched to make typos visible.
//!
//! Placeholders that produce markup (e.g. `{{pagination}}`) can take arguments:
//! `{{pagination class="pager" prev="Newer"}}`. Argument values are part of the
//! template, so they are inserted as written, like the rest of its HTML.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::rc::Rc;

/// What an argument accepts; checked by [`Template::problems`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Any text.
    Text,
    /// `true` or `false`.
    Bool,
    /// A heading element name, `h1` to `h6`.
    Heading,
}

/// The arguments a placeholder accepts.
pub type Spec = &'static [(&'static str, Kind)];

/// Arguments written in a placeholder, e.g. `class="pager"`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Args<'s> {
    pairs: Vec<(&'s str, &'s str)>,
}

impl<'s> Args<'s> {
    /// Parses `key="value" key2="value2"`.
    ///
    /// Values are kept as written: there is no escaping, so a value cannot
    /// contain `"`. A value cannot contain `}}` either, because the template
    /// scanner ends the placeholder at the first `}}` before this sees it.
    /// Names are limited to `a-z` and `_` so a typo such as `Class=` is reported
    /// instead of being silently ignored.
    pub fn parse(source: &'s str) -> Result<Self, String> {
        let mut pairs = Vec::new();
        let mut rest = source.trim_start();
        while !rest.is_empty() {
            let eq = rest
                .find('=')
                .ok_or_else(|| format!("expected key=\"value\" at {rest:?}"))?;
            let key = &rest[..eq];
            if key.is_empty() || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                return Err(format!("invalid argument name {key:?}"));
            }
            let value = rest[eq + 1..]
                .strip_prefix('"')
                .ok_or_else(|| format!("value of {key} must be in double quotes"))?;
            let end = value
                .find('"')
                .ok_or_else(|| format!("unclosed quote in {key}"))?;
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(format!("{key} is given twice"));
            }
            pairs.push((key, &value[..end]));
            rest = value[end + 1..].trim_start();
        }
        Ok(Self { pairs })
    }

    pub fn get(&self, key: &str) -> Option<&'s str> {
        self.pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    /// The value of `key`, or `default` when it is not given.
    pub fn text<'a>(&self, key: &str, default: &'a str) -> &'a str
    where
        's: 'a,
    {
        self.get(key).unwrap_or(default)
    }

    /// `key="true"` / `key="false"`, or `default` when it is not given.
    ///
    /// Any value other than `"true"` counts as false here; [`Template::problems`]
    /// warns about such values, so this only decides what is rendered meanwhile.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(value) => value == "true",
            None => default,
        }
    }

    fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

/// What a placeholder is replaced with.
///
/// `'a` is the lifetime of data a `Markup` closure borrows (posts, tag groups,
/// the page's root path). Site-wide values are built once as `Context<'static>`
/// and then extended per page; that works because `Context` is covariant in
/// `'a`, so a `Context<'static>` can be used where a shorter `'a` is expected.
#[derive(Clone)]
enum Value<'a> {
    /// Inserted as is; such placeholders take no arguments.
    Text(String),
    /// Rendered on each occurrence with that occurrence's arguments, so one
    /// template may use the same placeholder twice with different arguments.
    Markup {
        spec: Spec,
        // `Rc` because the shared base context is cloned for every page.
        render: Rc<dyn Fn(&Args) -> String + 'a>,
    },
}

/// Values available to a template, keyed by placeholder name.
#[derive(Default, Clone)]
pub struct Context<'a> {
    values: HashMap<String, Value<'a>>,
}

impl<'a> Context<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `key` to plain text, replacing any earlier value for it.
    /// The caller is responsible for escaping `value` when it is not HTML.
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.values
            .insert(key.to_string(), Value::Text(value.into()));
        self
    }

    /// Sets `key` to markup produced by `render`, which accepts the arguments
    /// in `spec`, replacing any earlier value for it. `render` receives only
    /// arguments that parsed; their names and values are checked separately by
    /// [`Template::problems`], so `render` must fall back to defaults for
    /// anything missing or out of range.
    pub fn with_markup(
        mut self,
        key: &str,
        spec: Spec,
        render: impl Fn(&Args) -> String + 'a,
    ) -> Self {
        let render = Rc::new(render);
        self.values
            .insert(key.to_string(), Value::Markup { spec, render });
        self
    }
}

/// A placeholder found in a template: its name and the text after the name.
struct Token<'s> {
    name: &'s str,
    args: &'s str,
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

    /// Placeholders in order of appearance. Stops at an unclosed `{{`, which
    /// [`Template::render`] also leaves alone.
    fn tokens(&self) -> impl Iterator<Item = Token<'_>> {
        let mut rest = self.source.as_str();
        std::iter::from_fn(move || {
            let start = rest.find("{{")?;
            let inner = &rest[start + 2..];
            let end = inner.find("}}")?;
            rest = &inner[end + 2..];
            Some(split_token(&inner[..end]))
        })
    }

    /// True if the template contains the placeholder `name`, with or without arguments.
    pub fn uses(&self, name: &str) -> bool {
        self.tokens().any(|token| token.name == name)
    }

    /// Mistakes in the template's placeholders: unknown names (usually typos),
    /// and arguments that are malformed, not accepted, or out of range.
    ///
    /// This is separate from [`Template::render`] so rendering never fails: a
    /// mistake becomes a build warning and the placeholder falls back to its
    /// default output. Each message appears once even if repeated.
    pub fn problems(&self, context: &Context) -> Vec<String> {
        let mut problems = Vec::new();
        for token in self.tokens() {
            let name = token.name;
            let found = match (context.values.get(name), Args::parse(token.args)) {
                (None, _) => vec![format!("unknown placeholder {{{{{name}}}}}")],
                // Parsing stops at the first error, so later arguments are not checked.
                (Some(_), Err(error)) => vec![format!("{{{{{name}}}}}: {error}")],
                (Some(Value::Text(_)), Ok(args)) if !args.is_empty() => {
                    vec![format!("{{{{{name}}}}} takes no arguments")]
                }
                (Some(Value::Text(_)), Ok(_)) => Vec::new(),
                (Some(Value::Markup { spec, .. }), Ok(args)) => check_args(name, spec, &args),
            };
            for problem in found {
                if !problems.contains(&problem) {
                    problems.push(problem);
                }
            }
        }
        problems
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
            let token = split_token(&inner[..end]);
            match context.values.get(token.name) {
                // Arguments on a text placeholder are ignored here and reported by `problems`.
                Some(Value::Text(value)) => out.push_str(value),
                Some(Value::Markup { render, .. }) => {
                    // Malformed arguments are reported by `problems`; render the default.
                    out.push_str(&render(&Args::parse(token.args).unwrap_or_default()));
                }
                None => out.push_str(&rest[start..start + 2 + end + 2]),
            }
            rest = &inner[end + 2..];
        }

        out.push_str(rest);
        out
    }
}

/// Splits the text between `{{` and `}}` into the name and the arguments.
/// Surrounding spaces are ignored, so `{{ title }}` and `{{title}}` are the same.
fn split_token(inner: &str) -> Token<'_> {
    let inner = inner.trim();
    match inner.find(char::is_whitespace) {
        Some(i) => Token {
            name: &inner[..i],
            args: &inner[i..],
        },
        None => Token {
            name: inner,
            args: "",
        },
    }
}

/// Checks parsed arguments against what the placeholder `name` accepts.
fn check_args(name: &str, spec: Spec, args: &Args) -> Vec<String> {
    let mut problems = Vec::new();
    for (key, value) in &args.pairs {
        let message = match spec.iter().find(|(k, _)| k == key) {
            None => {
                let accepted: Vec<&str> = spec.iter().map(|(k, _)| *k).collect();
                Some(match accepted.is_empty() {
                    true => format!("takes no arguments (got {key})"),
                    false => format!("unknown argument {key} (accepts {})", accepted.join(", ")),
                })
            }
            Some((_, Kind::Bool)) if !matches!(*value, "true" | "false") => Some(format!(
                "{key} must be \"true\" or \"false\" (got {value:?})"
            )),
            Some((_, Kind::Heading))
                if !matches!(*value, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") =>
            {
                Some(format!("{key} must be h1 to h6 (got {value:?})"))
            }
            Some(_) => None,
        };
        if let Some(message) = message {
            problems.push(format!("{{{{{name}}}}}: {message}"));
        }
    }
    problems
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
        assert_eq!(
            tpl.problems(&ctx),
            ["unknown placeholder {{a}}", "unknown placeholder {{b}}"]
        );
    }

    #[test]
    fn keeps_unknown_and_unclosed_tokens() {
        let tpl = Template::new("{{nope}} and {{open");
        assert_eq!(tpl.render(&Context::new()), "{{nope}} and {{open");
    }

    #[test]
    fn parses_arguments() {
        let args = Args::parse(r#" class="a b"  prev="<b>←</b>" empty="" "#).unwrap();
        assert_eq!(args.get("class"), Some("a b"));
        assert_eq!(args.get("prev"), Some("<b>←</b>"));
        assert_eq!(args.text("empty", "x"), "");
        assert_eq!(args.text("missing", "x"), "x");
        assert!(Args::parse("class=a").is_err());
        assert!(Args::parse("class=\"a").is_err());
        assert!(Args::parse("Class=\"a\"").is_err());
        assert!(Args::parse("a=\"1\" a=\"2\"").is_err());
    }

    #[test]
    fn renders_markup_with_arguments() {
        const SPEC: Spec = &[("class", Kind::Text)];
        let ctx = Context::new().with_markup("list", SPEC, |args| {
            format!("<ul class=\"{}\">", args.text("class", "list"))
        });
        let tpl = Template::new("{{list}}|{{ list class=\"x y\" }}");
        assert_eq!(tpl.render(&ctx), "<ul class=\"list\">|<ul class=\"x y\">");
        assert!(tpl.uses("list"));
        assert!(tpl.problems(&ctx).is_empty());
    }

    #[test]
    fn reports_bad_arguments() {
        const SPEC: Spec = &[("show", Kind::Bool), ("heading", Kind::Heading)];
        let ctx = Context::new()
            .with("title", "T")
            .with_markup("list", SPEC, |_| String::new());
        let tpl = Template::new(
            "{{title x=\"1\"}}{{list show=\"yes\" heading=\"h7\" size=\"2\"}}{{list show=1}}",
        );
        assert_eq!(
            tpl.problems(&ctx),
            [
                "{{title}} takes no arguments",
                "{{list}}: show must be \"true\" or \"false\" (got \"yes\")",
                "{{list}}: heading must be h1 to h6 (got \"h7\")",
                "{{list}}: unknown argument size (accepts show, heading)",
                "{{list}}: value of show must be in double quotes",
            ]
        );
    }
}
