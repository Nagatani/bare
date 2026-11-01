//! Build-time syntax highlighting for fenced code blocks.
//!
//! A small table-driven tokenizer covers the languages that show up most in
//! technical blogs. It recognises comments, strings, numbers, keywords, types,
//! function calls and a few language-specific forms, and emits
//! `<span class="hl-…">` that the theme colours. No JavaScript is shipped.

use crate::parser::escape_html;

/// Highlights `code` written in `lang`, returning escaped HTML, or `None` if the
/// language is not supported (the caller then emits plain escaped text).
pub fn highlight(lang: &str, code: &str) -> Option<String> {
    let lang = lang.to_ascii_lowercase();
    match lang.as_str() {
        "html" | "xml" | "svg" | "vue" => return Some(markup(code)),
        "diff" | "patch" => return Some(diff(code)),
        _ => {}
    }
    let spec = match lang.as_str() {
        "rust" | "rs" => &RUST,
        "toml" => &TOML,
        "yaml" | "yml" => &YAML,
        "json" | "jsonc" => &JSON,
        "sh" | "bash" | "shell" | "zsh" | "console" => &SHELL,
        "js" | "javascript" | "mjs" | "cjs" | "jsx" | "ts" | "typescript" | "tsx" => &JS,
        "python" | "py" => &PYTHON,
        "go" | "golang" => &GO,
        "c" | "h" | "cpp" | "c++" | "hpp" | "cc" | "java" | "kotlin" | "kt" | "swift" | "cs"
        | "csharp" => &C_LIKE,
        "css" | "scss" => &CSS,
        "sql" => &SQL,
        _ => return None,
    };
    Some(generic(spec, code))
}

/// Describes one language for the generic tokenizer.
struct Spec {
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    keywords: &'static [&'static str],
    literals: &'static [&'static str],
    types: &'static [&'static str],
    /// Treat capitalised identifiers as types (`String`, `HashMap`).
    caps_are_types: bool,
    /// Rust-style `name!` macros.
    macros: bool,
    /// Prefix of attributes/decorators: `#[` (Rust) or `@` (Python, TS, CSS at-rules).
    attribute: Option<&'static str>,
    /// `key:` / `key =` at the start of a line is a key (YAML, TOML, CSS properties).
    line_keys: bool,
    /// A string directly followed by `:` is a key (JSON).
    string_keys: bool,
    /// `[section]` at the start of a line (TOML).
    sections: bool,
    /// Shell `$VAR` / `${VAR}`.
    variables: bool,
    /// Comments only start at a word boundary (shell `#` inside URLs is not one).
    comment_needs_space: bool,
    /// `'a` is a lifetime unless it closes like a char literal (Rust).
    lifetimes: bool,
    /// `-` may appear inside identifiers (CSS, YAML).
    dash_in_idents: bool,
    case_insensitive: bool,
}

const BASE: Spec = Spec {
    line_comments: &[],
    block_comment: None,
    quotes: &['"', '\''],
    keywords: &[],
    literals: &[],
    types: &[],
    caps_are_types: false,
    macros: false,
    attribute: None,
    line_keys: false,
    string_keys: false,
    sections: false,
    variables: false,
    comment_needs_space: false,
    lifetimes: false,
    dash_in_idents: false,
    case_insensitive: false,
};

const RUST: Spec = Spec {
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type",
        "unsafe", "use", "where", "while",
    ],
    literals: &["true", "false"],
    types: &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str",
    ],
    caps_are_types: true,
    macros: true,
    attribute: Some("#"),
    lifetimes: true,
    ..BASE
};

const TOML: Spec = Spec {
    line_comments: &["#"],
    literals: &["true", "false"],
    line_keys: true,
    sections: true,
    dash_in_idents: true,
    ..BASE
};

const YAML: Spec = Spec {
    line_comments: &["#"],
    literals: &["true", "false", "null", "yes", "no", "on", "off"],
    line_keys: true,
    comment_needs_space: true,
    dash_in_idents: true,
    ..BASE
};

const JSON: Spec = Spec {
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    quotes: &['"'],
    literals: &["true", "false", "null"],
    string_keys: true,
    ..BASE
};

const SHELL: Spec = Spec {
    line_comments: &["#"],
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case",
        "esac", "function", "return", "export", "local", "readonly", "set", "unset",
    ],
    variables: true,
    comment_needs_space: true,
    dash_in_idents: true,
    ..BASE
};

const JS: Spec = Spec {
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    quotes: &['"', '\'', '`'],
    keywords: &[
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "finally",
        "for",
        "from",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "of",
        "private",
        "protected",
        "public",
        "readonly",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "try",
        "type",
        "typeof",
        "var",
        "void",
        "while",
        "yield",
    ],
    literals: &["true", "false", "null", "undefined", "NaN", "Infinity"],
    types: &[
        "string", "number", "boolean", "any", "unknown", "never", "object", "bigint",
    ],
    caps_are_types: true,
    attribute: Some("@"),
    ..BASE
};

const PYTHON: Spec = Spec {
    line_comments: &["#"],
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
        "lambda", "match", "case", "nonlocal", "not", "or", "pass", "raise", "return", "try",
        "while", "with", "yield", "self",
    ],
    literals: &["True", "False", "None"],
    types: &[
        "int", "float", "str", "bool", "list", "dict", "set", "tuple", "bytes",
    ],
    caps_are_types: true,
    attribute: Some("@"),
    ..BASE
};

const GO: Spec = Spec {
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    quotes: &['"', '\'', '`'],
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ],
    literals: &["true", "false", "nil", "iota"],
    types: &[
        "bool", "byte", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64",
        "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "any",
    ],
    ..BASE
};

const C_LIKE: Spec = Spec {
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    keywords: &[
        "abstract",
        "auto",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constexpr",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "extends",
        "extern",
        "final",
        "for",
        "fun",
        "func",
        "guard",
        "if",
        "implements",
        "import",
        "include",
        "interface",
        "let",
        "namespace",
        "new",
        "override",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "sizeof",
        "static",
        "struct",
        "switch",
        "template",
        "this",
        "throw",
        "throws",
        "try",
        "typedef",
        "typename",
        "union",
        "using",
        "val",
        "var",
        "virtual",
        "volatile",
        "when",
        "while",
    ],
    literals: &["true", "false", "null", "nullptr", "NULL", "nil"],
    types: &[
        "void", "int", "long", "short", "char", "float", "double", "bool", "boolean", "unsigned",
        "signed", "size_t", "string", "byte",
    ],
    caps_are_types: true,
    attribute: Some("@"),
    ..BASE
};

const CSS: Spec = Spec {
    block_comment: Some(("/*", "*/")),
    keywords: &["!important"],
    attribute: Some("@"),
    line_keys: true,
    dash_in_idents: true,
    ..BASE
};

const SQL: Spec = Spec {
    line_comments: &["--"],
    block_comment: Some(("/*", "*/")),
    quotes: &['\''],
    keywords: &[
        "select",
        "from",
        "where",
        "and",
        "or",
        "not",
        "insert",
        "into",
        "values",
        "update",
        "set",
        "delete",
        "create",
        "table",
        "drop",
        "alter",
        "add",
        "index",
        "primary",
        "key",
        "foreign",
        "references",
        "join",
        "left",
        "right",
        "inner",
        "outer",
        "on",
        "as",
        "group",
        "by",
        "order",
        "having",
        "limit",
        "offset",
        "distinct",
        "union",
        "all",
        "case",
        "when",
        "then",
        "else",
        "end",
        "in",
        "is",
        "like",
        "between",
        "exists",
        "with",
        "returning",
    ],
    literals: &["null", "true", "false"],
    types: &[
        "int",
        "integer",
        "bigint",
        "text",
        "varchar",
        "boolean",
        "date",
        "timestamp",
        "serial",
    ],
    case_insensitive: true,
    ..BASE
};

fn span(out: &mut String, class: &str, text: &str) {
    out.push_str("<span class=\"hl-");
    out.push_str(class);
    out.push_str("\">");
    out.push_str(&escape_html(text));
    out.push_str("</span>");
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char, spec: &Spec) -> bool {
    c.is_alphanumeric() || c == '_' || (spec.dash_in_idents && c == '-')
}

/// Byte length of the string literal starting at `s[0]` (the opening quote).
/// An unterminated string ends at the line break (template literals may span lines).
fn string_len(s: &str, quote: char) -> usize {
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '\n' if quote != '`' => return i,
            _ if c == quote => return i + c.len_utf8(),
            _ => {}
        }
    }
    s.len()
}

fn generic(spec: &Spec, code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut i = 0;
    // True while only whitespace has been seen on the current line.
    let mut line_start = true;
    let mut prev: Option<char> = None;

    while let Some(c) = code[i..].chars().next() {
        let rest = &code[i..];
        let after_boundary = prev.map_or(true, |p| p.is_whitespace() || p == ';' || p == '(');

        let token: Option<(&str, usize)> = (|| {
            if let Some((open, close)) = spec.block_comment {
                if let Some(body) = rest.strip_prefix(open) {
                    let end = body
                        .find(close)
                        .map_or(rest.len(), |e| open.len() + e + close.len());
                    return Some(("com", end));
                }
            }
            if spec.line_comments.iter().any(|m| rest.starts_with(m))
                && (!spec.comment_needs_space || after_boundary)
            {
                return Some(("com", rest.find('\n').unwrap_or(rest.len())));
            }
            if let Some(prefix) = spec.attribute {
                if prefix == "#" && (rest.starts_with("#[") || rest.starts_with("#![")) {
                    let line = rest.find('\n').unwrap_or(rest.len());
                    let end = rest[..line].rfind(']').map_or(line, |e| e + 1);
                    return Some(("mac", end));
                }
                if prefix == "@" && rest.starts_with('@') {
                    let len = 1 + rest[1..]
                        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '-')))
                        .unwrap_or(rest.len() - 1);
                    if len > 1 {
                        return Some(("mac", len));
                    }
                }
            }
            if spec.sections && line_start && c == '[' {
                let line = rest.find('\n').unwrap_or(rest.len());
                if let Some(end) = rest[..line].rfind(']') {
                    return Some(("mac", end + 1));
                }
            }
            if spec.quotes.contains(&c) {
                if spec.lifetimes && c == '\'' && !is_char_literal(rest) {
                    return None;
                }
                let len = string_len(rest, c);
                let is_key = spec.string_keys && rest[len..].trim_start().starts_with(':');
                return Some((if is_key { "key" } else { "str" }, len));
            }
            if spec.variables && c == '$' {
                let body = &rest[1..];
                let len = if body.starts_with('{') {
                    body.find('}').map_or(0, |e| e + 1)
                } else {
                    body.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .unwrap_or(body.len())
                };
                if len > 0 {
                    return Some(("var", len + 1));
                }
            }
            if c.is_ascii_digit() && !prev.is_some_and(|p| is_ident_char(p, spec)) {
                let mut len = 0;
                for (j, d) in rest.char_indices() {
                    // `1..2` is a range, not a float.
                    let is_range = d == '.' && rest[j + 1..].starts_with('.');
                    if !(d.is_alphanumeric() || d == '_' || d == '.') || is_range {
                        break;
                    }
                    len = j + d.len_utf8();
                }
                return Some(("num", len));
            }
            if is_ident_start(c) || (spec.keywords.contains(&"!important") && c == '!') {
                let len = rest
                    .char_indices()
                    .skip(1)
                    .find(|&(_, d)| !is_ident_char(d, spec))
                    .map_or(rest.len(), |(j, _)| j);
                let word = &rest[..len];
                let after = &rest[len..];
                let next = after.trim_start_matches([' ', '\t']).chars().next();
                let lookup = if spec.case_insensitive {
                    word.to_ascii_lowercase()
                } else {
                    word.to_string()
                };
                let class = if spec.keywords.contains(&lookup.as_str()) {
                    "kw"
                } else if spec.literals.contains(&lookup.as_str()) {
                    "lit"
                } else if spec.types.contains(&lookup.as_str()) {
                    "ty"
                } else if spec.line_keys && line_start && matches!(next, Some(':' | '=')) {
                    "key"
                } else if spec.macros && after.starts_with('!') && !after.starts_with("!=") {
                    return Some(("mac", len + 1));
                } else if spec.caps_are_types && word.starts_with(|c: char| c.is_uppercase()) {
                    "ty"
                } else if next == Some('(') {
                    "fn"
                } else {
                    ""
                };
                return Some((class, len));
            }
            None
        })();

        match token {
            Some((class, len)) if len > 0 => {
                let text = &rest[..len];
                if class.is_empty() {
                    out.push_str(&escape_html(text));
                } else {
                    span(&mut out, class, text);
                }
                prev = text.chars().last();
                line_start = false;
                i += len;
            }
            _ => {
                out.push_str(&escape_html(&rest[..c.len_utf8()]));
                if c == '\n' {
                    line_start = true;
                } else if !c.is_whitespace() {
                    line_start = false;
                }
                prev = Some(c);
                i += c.len_utf8();
            }
        }
    }
    out
}

/// `'x'` or `'\n'` (a char literal) as opposed to a lifetime like `'a`.
fn is_char_literal(s: &str) -> bool {
    let mut chars = s.chars().skip(1);
    match chars.next() {
        Some('\\') => true,
        Some(_) => chars.next() == Some('\''),
        None => false,
    }
}

/// HTML/XML: tags, attribute names, attribute values and comments.
fn markup(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut rest = code;
    while let Some(start) = rest.find('<') {
        out.push_str(&escape_html(&rest[..start]));
        rest = &rest[start..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(rest.len(), |e| e + 3);
            span(&mut out, "com", &rest[..end]);
            rest = &rest[end..];
            continue;
        }
        let end = rest.find('>').map_or(rest.len(), |e| e + 1);
        markup_tag(&mut out, &rest[..end]);
        rest = &rest[end..];
    }
    out.push_str(&escape_html(rest));
    out
}

fn markup_tag(out: &mut String, tag: &str) {
    let name_start = tag
        .char_indices()
        .find(|&(_, c)| !matches!(c, '<' | '/' | '!' | '?'))
        .map_or(tag.len(), |(i, _)| i);
    out.push_str(&escape_html(&tag[..name_start]));
    let body = &tag[name_start..];
    let name_len = body
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(body.len());
    span(out, "tag", &body[..name_len]);

    let mut rest = &body[name_len..];
    while let Some(c) = rest.chars().next() {
        if c == '"' || c == '\'' {
            let len = rest[1..].find(c).map_or(rest.len(), |e| e + 2);
            span(out, "str", &rest[..len]);
            rest = &rest[len..];
        } else if c.is_alphabetic() {
            let len = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/'))
                .unwrap_or(rest.len());
            span(out, "attr", &rest[..len]);
            rest = &rest[len..];
        } else {
            out.push_str(&escape_html(&rest[..c.len_utf8()]));
            rest = &rest[c.len_utf8()..];
        }
    }
}

/// Unified diffs: whole lines coloured as added, removed or metadata.
fn diff(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    for (n, line) in code.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let class = if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@")
        {
            "meta"
        } else if line.starts_with('+') {
            "add"
        } else if line.starts_with('-') {
            "del"
        } else {
            ""
        };
        if class.is_empty() {
            out.push_str(&escape_html(line));
        } else {
            span(&mut out, class, line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust() {
        let html = highlight(
            "rust",
            "#[derive(Debug)]\nfn main() { let v: Vec<u8> = vec![1]; // hi\n}",
        )
        .unwrap();
        assert_eq!(
            html,
            "<span class=\"hl-mac\">#[derive(Debug)]</span>\n\
             <span class=\"hl-kw\">fn</span> <span class=\"hl-fn\">main</span>() { \
             <span class=\"hl-kw\">let</span> v: <span class=\"hl-ty\">Vec</span>&lt;<span class=\"hl-ty\">u8</span>&gt; = \
             <span class=\"hl-mac\">vec!</span>[<span class=\"hl-num\">1</span>]; <span class=\"hl-com\">// hi</span>\n}"
        );
    }

    #[test]
    fn distinguishes_lifetimes_from_chars() {
        let html = highlight("rust", "fn f<'a>(c: char) { 'x' }").unwrap();
        assert!(html.contains("&lt;&#39;a&gt;"));
        assert!(html.contains("<span class=\"hl-str\">&#39;x&#39;</span>"));
    }

    #[test]
    fn highlights_keys_and_sections() {
        assert_eq!(
            highlight("toml", "[package]\nname = \"bare\" # c").unwrap(),
            "<span class=\"hl-mac\">[package]</span>\n<span class=\"hl-key\">name</span> = \
             <span class=\"hl-str\">&quot;bare&quot;</span> <span class=\"hl-com\"># c</span>"
        );
        assert_eq!(
            highlight("json", "{\"a\": true}").unwrap(),
            "{<span class=\"hl-key\">&quot;a&quot;</span>: <span class=\"hl-lit\">true</span>}"
        );
    }

    #[test]
    fn shell_comments_need_a_boundary() {
        assert_eq!(
            highlight("sh", "curl https://x/#a # c $HOME").unwrap(),
            "curl https://x/#a <span class=\"hl-com\"># c $HOME</span>"
        );
        assert_eq!(
            highlight("sh", "echo $HOME").unwrap(),
            "echo <span class=\"hl-var\">$HOME</span>"
        );
    }

    #[test]
    fn highlights_markup_and_diff() {
        assert_eq!(
            highlight("html", "<a href=\"x\">t</a>").unwrap(),
            "&lt;<span class=\"hl-tag\">a</span> <span class=\"hl-attr\">href</span>=\
             <span class=\"hl-str\">&quot;x&quot;</span>&gt;t&lt;/<span class=\"hl-tag\">a</span>&gt;"
        );
        assert_eq!(
            highlight("diff", "@@ x\n-a\n+b\n c").unwrap(),
            "<span class=\"hl-meta\">@@ x</span>\n<span class=\"hl-del\">-a</span>\n\
             <span class=\"hl-add\">+b</span>\n c"
        );
    }

    #[test]
    fn unknown_language_is_none() {
        assert!(highlight("brainfuck", "+").is_none());
    }
}
