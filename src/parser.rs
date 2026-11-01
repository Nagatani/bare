//! Frontmatter and minimal Markdown parsing.
//!
//! Everything here is plain string slicing over lines — no regex, no external
//! Markdown crate. Supported blocks: headings, paragraphs, fenced code blocks,
//! ordered/unordered (nested, task) lists, blockquotes, tables, thematic breaks
//! and footnotes. Supported inlines: code, links, images, `**strong**`, `*em*`,
//! `~~del~~`, footnote references `[^1]` and `\` escapes.

use std::collections::HashMap;

use crate::highlight;

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
/// A heading of the document, collected for the table of contents.
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    pub level: usize,
    pub id: String,
    /// Rendered inline HTML of the heading text (without the `#` anchor).
    pub html: String,
}

/// The HTML of a document plus what was collected while rendering it.
#[derive(Debug)]
pub struct Rendered {
    pub html: String,
    /// Top-level headings in document order.
    pub headings: Vec<Heading>,
}

/// Converts the supported Markdown subset to HTML.
///
/// `root` is the relative path from the rendered page to the site root (e.g. `"../"`).
/// Link and image URLs starting with a single `/` are rewritten against it, so
/// `/images/a.png` keeps working when the site is served from a sub-path.
#[cfg(test)]
pub fn markdown_to_html(markdown: &str, root: &str) -> String {
    render_markdown(markdown, root).html
}

/// Like [`markdown_to_html`], also returning the headings for a table of contents.
pub fn render_markdown(markdown: &str, root: &str) -> Rendered {
    // NUL marks footnote references until they are numbered (see `finish_footnotes`).
    let markdown = markdown.replace('\0', "");
    let mut state = State {
        root,
        ids: HashMap::new(),
        headings: Vec::new(),
        footnotes: Vec::new(),
        depth: 0,
    };
    let body = render_blocks(&markdown, &mut state);
    let html = finish_footnotes(body, &state);
    Rendered {
        html,
        headings: state.headings,
    }
}

/// Values shared by a document and the blocks nested inside it (lists, quotes).
struct State<'r> {
    root: &'r str,
    /// Heading ids used so far, for de-duplication.
    ids: HashMap<String, usize>,
    headings: Vec<Heading>,
    /// Footnote definitions as (label, Markdown text), in source order.
    footnotes: Vec<(String, String)>,
    /// Nesting level: 0 for the document itself, more inside lists and quotes.
    depth: usize,
}

fn render_blocks(markdown: &str, state: &mut State) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks = Blocks {
        out: String::with_capacity(markdown.len() * 2),
        state,
        paragraph: Vec::new(),
    };

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        if let Some(fence) = parse_fence(trimmed) {
            blocks.flush();
            let start = i + 1;
            let mut end = start;
            while end < lines.len() && !is_closing_fence(lines[end].trim(), fence.len) {
                end += 1;
            }
            // An unterminated fence renders everything after it as code.
            blocks.code_block(fence.lang, &lines[start..end]);
            i = end + 1;
            continue;
        }

        if trimmed.is_empty() {
            blocks.flush();
        } else if let Some((label, text)) = parse_footnote_def(trimmed) {
            blocks.flush();
            // Indented lines right after the definition continue it.
            let mut parts = vec![text];
            i += 1;
            while i < lines.len() && indent_of(lines[i]) >= 2 && !lines[i].trim().is_empty() {
                parts.push(lines[i].trim());
                i += 1;
            }
            if !blocks.state.footnotes.iter().any(|(l, _)| l == label) {
                let text = join_lines(&parts);
                blocks.state.footnotes.push((label.to_string(), text));
            }
            continue;
        } else if let Some((level, text)) = parse_heading(trimmed) {
            blocks.flush();
            blocks.heading(level, text);
        } else if is_thematic_break(trimmed) {
            blocks.flush();
            blocks.out.push_str("<hr>\n");
        } else if trimmed.starts_with('>') {
            blocks.flush();
            let mut inner = Vec::new();
            while i < lines.len() {
                let Some(rest) = lines[i].trim().strip_prefix('>') else {
                    break;
                };
                inner.push(rest.strip_prefix(' ').unwrap_or(rest));
                i += 1;
            }
            let html = blocks.nested(&inner.join("\n"));
            blocks.out.push_str("<blockquote>\n");
            blocks.out.push_str(&html);
            blocks.out.push_str("</blockquote>\n");
            continue;
        } else if let Some(aligns) = table_alignments(trimmed, lines.get(i + 1).copied()) {
            blocks.flush();
            let mut end = i + 2;
            while end < lines.len() && lines[end].trim().starts_with('|') {
                end += 1;
            }
            blocks.table(trimmed, &aligns, &lines[i + 2..end]);
            i = end;
            continue;
        } else if parse_list_item(trimmed).is_some() {
            blocks.flush();
            let end = list_end(&lines, i);
            blocks.list(&lines[i..end]);
            i = end;
            continue;
        } else {
            blocks.paragraph.push(trimmed);
        }
        i += 1;
    }

    blocks.flush();
    blocks.out
}

/// Output buffer plus the paragraph currently being collected.
struct Blocks<'a, 's, 'r> {
    out: String,
    state: &'s mut State<'r>,
    paragraph: Vec<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ListKind {
    Unordered,
    /// Ordered list with the number of its first item.
    Ordered(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Align {
    None,
    Left,
    Center,
    Right,
}

impl Blocks<'_, '_, '_> {
    fn flush(&mut self) {
        if self.paragraph.is_empty() {
            return;
        }
        let text = join_lines(&self.paragraph);
        self.out.push_str("<p>");
        self.out.push_str(&render_inline(&text, self.state.root));
        self.out.push_str("</p>\n");
        self.paragraph.clear();
    }

    /// Renders Markdown nested inside a list item or blockquote.
    fn nested(&mut self, markdown: &str) -> String {
        self.state.depth += 1;
        let html = render_blocks(markdown, self.state);
        self.state.depth -= 1;
        html
    }

    /// Emits a heading with an `id` and a `#` self-link, so sections can be
    /// linked to directly (e.g. `posts/a.html#インストール`).
    fn heading(&mut self, level: usize, text: &str) {
        let html = render_inline(text, self.state.root);
        let id = self.unique_id(slugify(&strip_tags(&html)));
        self.out.push_str(&format!(
            "<h{level} id=\"{id}\">{html}<a class=\"heading-anchor\" href=\"#{id}\" \
             aria-hidden=\"true\" tabindex=\"-1\">#</a></h{level}>\n",
            id = escape_html(&id),
        ));
        if self.state.depth == 0 {
            self.state.headings.push(Heading { level, id, html });
        }
    }

    fn unique_id(&mut self, base: String) -> String {
        let base = if base.is_empty() {
            "section".to_string()
        } else {
            base
        };
        let seen = self.state.ids.entry(base.clone()).or_insert(0);
        *seen += 1;
        match *seen {
            1 => base,
            n => format!("{base}-{}", n - 1),
        }
    }

    /// Renders the lines of one list region (see [`list_end`]). Lines indented
    /// past the item marker belong to the item and may hold nested lists, code
    /// blocks or further paragraphs.
    fn list(&mut self, lines: &[&str]) {
        let base = indent_of(lines[0]);
        let mut items: Vec<(ListKind, &str, Vec<&str>)> = Vec::new();
        for &line in lines {
            let marker = (indent_of(line) <= base)
                .then(|| parse_list_item(line.trim()))
                .flatten();
            match (marker, items.last_mut()) {
                (Some((kind, text)), _) => items.push((kind, text, Vec::new())),
                (None, Some((_, _, children))) => children.push(line),
                (None, None) => {}
            }
        }

        let mut open: Option<ListKind> = None;
        for (kind, text, children) in items {
            let same_kind = matches!(
                (open, kind),
                (Some(ListKind::Unordered), ListKind::Unordered)
                    | (Some(ListKind::Ordered(_)), ListKind::Ordered(_))
            );
            if !same_kind {
                self.close_list(open);
                self.out.push_str(&match kind {
                    ListKind::Unordered => "<ul>\n".to_string(),
                    ListKind::Ordered(1) => "<ol>\n".to_string(),
                    ListKind::Ordered(start) => format!("<ol start=\"{start}\">\n"),
                });
                open = Some(kind);
            }

            // Plain lines right below the item continue its text.
            let mut text_lines = vec![text];
            let mut k = 0;
            while k < children.len() {
                let line = children[k].trim();
                if line.is_empty() || starts_block(line) {
                    break;
                }
                text_lines.push(line);
                k += 1;
            }
            let text = join_lines(&text_lines);
            let (class, text) = match task_marker(&text) {
                Some((done, rest)) => (
                    " class=\"task\"",
                    format!(
                        "<label><input type=\"checkbox\" disabled{}> {}</label>",
                        if done { " checked" } else { "" },
                        render_inline(rest, self.state.root)
                    ),
                ),
                None => ("", render_inline(&text, self.state.root)),
            };
            self.out.push_str(&format!("  <li{class}>{text}"));
            let rest = &children[k..];
            if rest.iter().any(|line| !line.trim().is_empty()) {
                let html = self.nested(&dedent(rest));
                self.out.push('\n');
                self.out.push_str(&html);
            }
            self.out.push_str("</li>\n");
        }
        self.close_list(open);
    }

    fn close_list(&mut self, kind: Option<ListKind>) {
        match kind {
            Some(ListKind::Unordered) => self.out.push_str("</ul>\n"),
            Some(ListKind::Ordered(_)) => self.out.push_str("</ol>\n"),
            None => {}
        }
    }

    fn code_block(&mut self, lang: &str, lines: &[&str]) {
        if lang.is_empty() {
            self.out.push_str("<pre><code>");
        } else {
            self.out.push_str(&format!(
                "<pre><code class=\"language-{}\">",
                escape_html(lang)
            ));
        }
        let code = lines.join("\n");
        let html = highlight::highlight(lang, &code).unwrap_or_else(|| escape_html(&code));
        self.out.push_str(&html);
        self.out.push_str("</code></pre>\n");
    }

    fn table(&mut self, header: &str, aligns: &[Align], rows: &[&str]) {
        self.out
            .push_str("<div class=\"table-wrap\"><table>\n<thead>\n");
        self.table_row(header, aligns, "th");
        self.out.push_str("</thead>\n<tbody>\n");
        for row in rows {
            self.table_row(row, aligns, "td");
        }
        self.out.push_str("</tbody>\n</table></div>\n");
    }

    fn table_row(&mut self, row: &str, aligns: &[Align], tag: &str) {
        let cells = split_row(row);
        self.out.push_str("<tr>");
        // Rows are padded or truncated to the header's column count.
        for (col, align) in aligns.iter().enumerate() {
            let cell = cells.get(col).copied().unwrap_or("");
            let style = match align {
                Align::None => "",
                Align::Left => " style=\"text-align: left\"",
                Align::Center => " style=\"text-align: center\"",
                Align::Right => " style=\"text-align: right\"",
            };
            // `\|` is a literal pipe in a cell, even inside a code span.
            let html = render_inline(&cell.replace("\\|", "|"), self.state.root);
            self.out.push_str(&format!("<{tag}{style}>{html}</{tag}>"));
        }
        self.out.push_str("</tr>\n");
    }
}

/// Returns the end (exclusive) of the list starting at `lines[start]`: following
/// list items and lines indented deeper than the first marker belong to it, also
/// across blank lines. A non-indented line of other text ends the list.
fn list_end(lines: &[&str], start: usize) -> usize {
    let base = indent_of(lines[start]);
    let belongs = |line: &str| indent_of(line) > base || parse_list_item(line.trim()).is_some();
    let mut end = start + 1;
    let mut i = start + 1;
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            match (i + 1..lines.len()).find(|&k| !lines[k].trim().is_empty()) {
                Some(next) if belongs(lines[next]) => i = next,
                _ => break,
            }
        }
        if !belongs(lines[i]) {
            break;
        }
        i += 1;
        end = i;
    }
    end
}

/// Lines inside a list item that start their own block rather than continue the text.
fn starts_block(line: &str) -> bool {
    parse_list_item(line).is_some()
        || parse_fence(line).is_some()
        || parse_heading(line).is_some()
        || line.starts_with('>')
        || line.starts_with('|')
}

/// `[ ] text` / `[x] text` at the start of a list item (a task list).
fn task_marker(text: &str) -> Option<(bool, &str)> {
    let done = match text.get(..4)? {
        "[ ] " => false,
        "[x] " | "[X] " => true,
        _ => return None,
    };
    Some((done, &text[4..]))
}

/// Width of the leading whitespace, counting a tab as four spaces.
fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Removes the common leading indentation from `lines`.
fn dedent(lines: &[&str]) -> String {
    let width = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| indent_of(line))
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for line in lines {
        let mut removed = 0;
        let mut rest = *line;
        while removed < width {
            match rest.chars().next() {
                Some(' ') => removed += 1,
                Some('\t') => removed += 4,
                _ => break,
            }
            rest = &rest[1..];
        }
        out.push_str(rest);
        out.push('\n');
    }
    out
}

/// Recognises a footnote definition `[^label]: text`.
fn parse_footnote_def(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("[^")?;
    let (label, text) = rest.split_once("]:")?;
    let valid = !label.is_empty() && !label.contains(|c: char| c.is_whitespace() || c == ']');
    valid.then(|| (label, text.trim()))
}

/// Start and end of the placeholder `render_inline` writes for `[^label]`.
const FOOTNOTE_OPEN: &str = "\0fn:";
const FOOTNOTE_CLOSE: char = '\0';

/// Numbers footnote references in the order they appear, links them to their
/// definitions, and appends the footnote list. References without a definition
/// stay as literal text; definitions that are never referenced are left out.
fn finish_footnotes(body: String, state: &State) -> String {
    if !body.contains(FOOTNOTE_OPEN) {
        return body;
    }
    let defined = |label: &str| state.footnotes.iter().any(|(l, _)| l == label);
    let mut order: Vec<&str> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let body = replace_footnote_refs(&body, |label| {
        if !defined(label) {
            return None;
        }
        let n = match order.iter().position(|l| *l == label) {
            Some(index) => index,
            None => {
                order.push(state.footnotes.iter().find(|(l, _)| l == label)?.0.as_str());
                counts.push(0);
                order.len() - 1
            }
        };
        counts[n] += 1;
        let number = n + 1;
        let id = match counts[n] {
            1 => format!("fnref-{number}"),
            count => format!("fnref-{number}-{count}"),
        };
        Some(format!(
            "<sup class=\"footnote-ref\"><a href=\"#fn-{number}\" id=\"{id}\" \
             role=\"doc-noteref\">{number}</a></sup>"
        ))
    });

    let mut out = body;
    out.push_str("<section class=\"footnotes\" role=\"doc-endnotes\">\n<ol>\n");
    for (index, label) in order.iter().enumerate() {
        let number = index + 1;
        let text = state
            .footnotes
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, text)| text.as_str())
            .unwrap_or_default();
        // A reference inside a footnote links to its note but gets no number of its own.
        let html = replace_footnote_refs(&render_inline(text, state.root), |inner| {
            let n = order.iter().position(|l| *l == inner)? + 1;
            Some(format!(
                "<sup class=\"footnote-ref\"><a href=\"#fn-{n}\">{n}</a></sup>"
            ))
        });
        out.push_str(&format!(
            "  <li id=\"fn-{number}\">{html} <a href=\"#fnref-{number}\" class=\"footnote-back\" \
             role=\"doc-backlink\">↩\u{fe0e}</a></li>\n"
        ));
    }
    out.push_str("</ol>\n</section>\n");
    out
}

/// Replaces each footnote placeholder with `link(label)`, or with the literal
/// `[^label]` when that returns `None`.
fn replace_footnote_refs(html: &str, mut link: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(FOOTNOTE_OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + FOOTNOTE_OPEN.len()..];
        let end = after.find(FOOTNOTE_CLOSE).unwrap_or(after.len());
        let label = &after[..end];
        match link(label) {
            Some(html) => out.push_str(&html),
            None => out.push_str(&escape_html(&format!("[^{label}]"))),
        }
        rest = after.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// Renders a nested `<ol>` table of contents from the two highest heading levels
/// present. Returns an empty string for fewer than two entries.
pub fn render_toc(headings: &[Heading]) -> String {
    let Some(top) = headings.iter().map(|h| h.level).min() else {
        return String::new();
    };
    let entries: Vec<&Heading> = headings.iter().filter(|h| h.level <= top + 1).collect();
    if entries.len() < 2 {
        return String::new();
    }
    let mut out = String::from("<ol class=\"toc\">\n");
    let mut sub_open = false;
    let mut item_open = false;
    for heading in entries {
        let link = format!(
            "<a href=\"#{}\">{}</a>",
            escape_html(&heading.id),
            toc_text(&heading.html)
        );
        if heading.level > top && item_open {
            if !sub_open {
                out.push_str("\n<ol>\n");
                sub_open = true;
            }
            out.push_str(&format!("<li>{link}</li>\n"));
            continue;
        }
        if sub_open {
            out.push_str("</ol>\n");
            sub_open = false;
        }
        if item_open {
            out.push_str("</li>\n");
        }
        out.push_str(&format!("<li>{link}"));
        item_open = true;
    }
    if sub_open {
        out.push_str("</ol>\n");
    }
    if item_open {
        out.push_str("</li>\n");
    }
    out.push_str("</ol>");
    out
}

/// Heading HTML for use inside a TOC link: nested links and footnote
/// placeholders removed, other inline markup (e.g. `<code>`) kept.
fn toc_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        if rest.starts_with("<a ") || rest.starts_with("</a>") {
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            rest = &rest[end..];
        } else if rest.starts_with(FOOTNOTE_OPEN) {
            let after = &rest[FOOTNOTE_OPEN.len()..];
            let end = after.find(FOOTNOTE_CLOSE).map_or(after.len(), |i| i + 1);
            rest = &after[end..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

struct Fence<'a> {
    len: usize,
    lang: &'a str,
}

/// Recognises an opening fence of three or more backticks, e.g. ```` ```rust ````.
fn parse_fence(line: &str) -> Option<Fence<'_>> {
    let len = line.bytes().take_while(|&b| b == b'`').count();
    if len < 3 {
        return None;
    }
    Some(Fence {
        len,
        lang: line[len..].trim(),
    })
}

/// A closing fence is only backticks, at least as many as the opening one.
fn is_closing_fence(line: &str, open_len: usize) -> bool {
    line.len() >= open_len && line.bytes().all(|b| b == b'`')
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

/// `---`, `***` or `___` (three or more, spaces allowed in between).
fn is_thematic_break(line: &str) -> bool {
    let Some(marker) = line.chars().next() else {
        return false;
    };
    matches!(marker, '-' | '*' | '_')
        && line.chars().filter(|&c| c == marker).count() >= 3
        && line.chars().all(|c| c == marker || c == ' ')
}

/// Recognises `- item`, `* item`, `+ item` and `1. item` / `1) item`.
fn parse_list_item(line: &str) -> Option<(ListKind, &str)> {
    for bullet in ["- ", "* ", "+ "] {
        if let Some(item) = line.strip_prefix(bullet) {
            return Some((ListKind::Unordered, item.trim()));
        }
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = &line[digits..];
    let item = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    let start = line[..digits].parse().ok()?;
    Some((ListKind::Ordered(start), item.trim()))
}

/// Returns column alignments when `line` is a table header followed by a
/// delimiter row such as `| --- | :---: | ---: |`.
fn table_alignments(line: &str, next: Option<&str>) -> Option<Vec<Align>> {
    if !line.starts_with('|') {
        return None;
    }
    let next = next?.trim();
    if !next.starts_with('|') {
        return None;
    }
    split_row(next)
        .into_iter()
        .map(|cell| {
            let dashes = cell.trim_start_matches(':').trim_end_matches(':');
            if dashes.is_empty() || !dashes.bytes().all(|b| b == b'-') {
                return None;
            }
            Some(match (cell.starts_with(':'), cell.ends_with(':')) {
                (true, true) => Align::Center,
                (true, false) => Align::Left,
                (false, true) => Align::Right,
                (false, false) => Align::None,
            })
        })
        .collect()
}

/// Splits `| a | b |` into trimmed cells. A `|` inside backticks or escaped as
/// `\|` does not split.
fn split_row(row: &str) -> Vec<&str> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);

    let mut cells = Vec::new();
    let mut start = 0;
    let mut in_code = false;
    let mut escaped = false;
    for (i, c) in row.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '`' => in_code = !in_code,
            '|' if !in_code => {
                cells.push(row[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    cells.push(row[start..].trim());
    cells
}

/// Removes tags and character references from rendered inline HTML.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut skip_until = None;
    for c in html.chars() {
        match (skip_until, c) {
            (Some(end), _) if c == end => skip_until = None,
            (Some(_), _) => {}
            (None, '<') => skip_until = Some('>'),
            (None, FOOTNOTE_CLOSE) => skip_until = Some(FOOTNOTE_CLOSE),
            (None, '&') => skip_until = Some(';'),
            (None, _) => out.push(c),
        }
    }
    out
}

/// Turns heading text into an id: lowercase letters and digits (any script)
/// are kept, whitespace and `-`/`_` become single hyphens, the rest is dropped.
fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_')
            && !out.is_empty()
            && !out.ends_with('-')
        {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Joins the lines of a paragraph. No space is inserted when either side of the
/// line break is a CJK (or other wide) character, so Japanese text flows naturally.
fn join_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines {
        let needs_space = match (out.chars().last(), line.chars().next()) {
            (Some(prev), Some(next)) => !is_wide(prev) && !is_wide(next),
            _ => false,
        };
        if needs_space {
            out.push(' ');
        }
        out.push_str(line);
    }
    out
}

/// Rough check for characters of CJK scripts and full-width forms.
fn is_wide(c: char) -> bool {
    c >= '\u{2E80}'
}

/// Renders inline code, images, links, emphasis and backslash escapes;
/// everything else is HTML-escaped text.
fn render_inline(text: &str, root: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(c) = rest.chars().next() {
        match c {
            '\\' => {
                if let Some(next) = rest[1..].chars().next().filter(char::is_ascii_punctuation) {
                    push_escaped(&mut out, next);
                    rest = &rest[1 + next.len_utf8()..];
                    continue;
                }
            }
            '`' => {
                // A code span opens and closes with backtick runs of the same length,
                // so ``` `` `code` `` ``` can contain single backticks.
                let run = rest.bytes().take_while(|&b| b == b'`').count();
                match find_backtick_run(&rest[run..], run) {
                    Some(end) => {
                        out.push_str("<code>");
                        out.push_str(&escape_html(strip_code_padding(&rest[run..run + end])));
                        out.push_str("</code>");
                        rest = &rest[run + end + run..];
                    }
                    None => {
                        out.push_str(&rest[..run]);
                        rest = &rest[run..];
                    }
                }
                continue;
            }
            '!' => {
                if let Some(link) = rest[1..]
                    .starts_with('[')
                    .then(|| parse_link(&rest[1..]))
                    .flatten()
                {
                    out.push_str(&format!(
                        "<img src=\"{}\" alt=\"{}\"{} loading=\"lazy\">",
                        escape_html(&resolve_url(link.url, root)),
                        escape_html(link.label),
                        title_attr(link.title),
                    ));
                    rest = &rest[1 + link.consumed..];
                    continue;
                }
            }
            '[' => {
                if let Some(label) = footnote_ref(rest) {
                    out.push_str(FOOTNOTE_OPEN);
                    out.push_str(label);
                    out.push(FOOTNOTE_CLOSE);
                    rest = &rest[label.len() + 3..];
                    continue;
                }
                if let Some(link) = parse_link(rest) {
                    out.push_str(&format!(
                        "<a href=\"{}\"{}>{}</a>",
                        escape_html(&resolve_url(link.url, root)),
                        title_attr(link.title),
                        render_inline(link.label, root)
                    ));
                    rest = &rest[link.consumed..];
                    continue;
                }
            }
            '*' | '~' => {
                let (marker, tag) = if rest.starts_with("**") {
                    ("**", "strong")
                } else if rest.starts_with("~~") {
                    ("~~", "del")
                } else if c == '*' {
                    ("*", "em")
                } else {
                    ("", "")
                };
                if !marker.is_empty() {
                    let body = &rest[marker.len()..];
                    if let Some(end) = find_closing(body, marker) {
                        let inner = render_inline(&body[..end], root);
                        out.push_str(&format!("<{tag}>{inner}</{tag}>"));
                        rest = &body[end + marker.len()..];
                        continue;
                    }
                }
            }
            _ => {}
        }
        push_escaped(&mut out, c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Label of a footnote reference `[^label]` at the start of `s`.
fn footnote_ref(s: &str) -> Option<&str> {
    let rest = s.strip_prefix("[^")?;
    let label = &rest[..rest.find(']')?];
    let valid = !label.is_empty() && !label.contains(|c: char| c.is_whitespace() || c == '[');
    // `[^x](url)` is a link, not a footnote.
    let is_link = rest[label.len() + 1..].starts_with('(');
    (valid && !is_link).then_some(label)
}

/// Finds the byte offset of a run of exactly `len` backticks in `s`.
fn find_backtick_run(s: &str, len: usize) -> Option<usize> {
    let mut from = 0;
    while let Some(pos) = s[from..].find('`') {
        let start = from + pos;
        let run = s[start..].bytes().take_while(|&b| b == b'`').count();
        if run == len {
            return Some(start);
        }
        from = start + run;
    }
    None
}

/// Strips one leading and trailing space, as in `` `` `x` `` `` → `` `x` ``.
fn strip_code_padding(code: &str) -> &str {
    if code.len() >= 2 && code.starts_with(' ') && code.ends_with(' ') && !code.trim().is_empty() {
        &code[1..code.len() - 1]
    } else {
        code
    }
}

/// Finds the closing emphasis `marker`; the enclosed text must be non-empty and
/// must not start or end with whitespace (so `2 * 3 * 4` stays literal).
fn find_closing(s: &str, marker: &str) -> Option<usize> {
    let end = s.find(marker)?;
    let inner = &s[..end];
    let valid = !inner.is_empty()
        && !inner.starts_with(char::is_whitespace)
        && !inner.ends_with(char::is_whitespace);
    valid.then_some(end)
}

struct Link<'a> {
    label: &'a str,
    url: &'a str,
    title: Option<&'a str>,
    consumed: usize,
}

/// Parses `[label](url "title")` at the start of `s`. Balanced parentheses are
/// allowed inside the URL, e.g. `https://en.wikipedia.org/wiki/Rust_(programming_language)`.
fn parse_link(s: &str) -> Option<Link<'_>> {
    let close = s.find(']')?;
    let after = s[close + 1..].strip_prefix('(')?;

    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in after.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                end = Some(i);
                break;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    let end = end?;

    let target = after[..end].trim();
    let (url, title) = match target.split_once(char::is_whitespace) {
        Some((url, title)) => (url, Some(unquote(title.trim()))),
        None => (target, None),
    };
    Some(Link {
        label: &s[1..close],
        url,
        title,
        consumed: close + 2 + end + 1,
    })
}

/// Rewrites site-absolute URLs (`/images/a.png`) relative to `root`.
fn resolve_url(url: &str, root: &str) -> String {
    match url.strip_prefix('/') {
        Some(path) if !path.starts_with('/') => format!("{root}{path}"),
        _ => url.to_string(),
    }
}

fn title_attr(title: Option<&str>) -> String {
    title
        .map(|t| format!(" title=\"{}\"", escape_html(t)))
        .unwrap_or_default()
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

    fn md(src: &str) -> String {
        markdown_to_html(src, "../")
    }

    #[test]
    fn parses_frontmatter() {
        let doc =
            parse_document("---\ntitle: \"Hi: there\"\ndate: 2026-01-02\ntags: [a, b]\n---\nBody");
        assert_eq!(doc.frontmatter.get("title"), Some("Hi: there"));
        assert_eq!(doc.frontmatter.get("date"), Some("2026-01-02"));
        assert_eq!(doc.frontmatter.tags(), vec!["a", "b"]);
        assert_eq!(doc.body, "Body");
    }

    #[test]
    fn parses_key_value_file() {
        let conf =
            parse_key_values("# comment: ignored\ntitle: My Blog\n\nurl: https://example.com\n");
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
        let src = "line a\nline b\n\n- x\n- `y`\n";
        assert_eq!(
            md(src),
            "<p>line a line b</p>\n<ul>\n  <li>x</li>\n  <li><code>y</code></li>\n</ul>\n"
        );
    }

    #[test]
    fn renders_headings_with_ids() {
        assert_eq!(
            md("## Hello, `World`!"),
            "<h2 id=\"hello-world\">Hello, <code>World</code>!<a class=\"heading-anchor\" \
             href=\"#hello-world\" aria-hidden=\"true\" tabindex=\"-1\">#</a></h2>\n"
        );
        let html = md("# Rust & Go\n## はじめに 1\n## はじめに 1\n### !!!");
        assert!(html.contains("id=\"rust-go\""));
        assert!(html.contains("id=\"はじめに-1\""));
        assert!(html.contains("id=\"はじめに-1-1\""));
        assert!(html.contains("id=\"section\""));
    }

    #[test]
    fn joins_cjk_lines_without_space() {
        assert_eq!(
            md("です。\n次の行\nEnglish\ntext"),
            "<p>です。次の行English text</p>\n"
        );
    }

    #[test]
    fn escapes_code_blocks() {
        // A language without a highlighter: plain, escaped text.
        let src = "```text\nlet v: Vec<u8> = vec![];\n  {{title}}\n```";
        assert_eq!(
            md(src),
            "<pre><code class=\"language-text\">let v: Vec&lt;u8&gt; = vec![];\n  {{title}}</code></pre>\n"
        );
    }

    #[test]
    fn longer_fence_can_contain_backticks() {
        assert_eq!(
            md("````md\n```\ninner\n```\n````"),
            "<pre><code class=\"language-md\">```\ninner\n```</code></pre>\n"
        );
    }

    #[test]
    fn renders_ordered_lists() {
        assert_eq!(
            md("1. a\n2. b\n- c"),
            "<ol>\n  <li>a</li>\n  <li>b</li>\n</ol>\n<ul>\n  <li>c</li>\n</ul>\n"
        );
        assert_eq!(md("3) x"), "<ol start=\"3\">\n  <li>x</li>\n</ol>\n");
    }

    #[test]
    fn renders_blockquote_and_rule() {
        assert_eq!(
            md("> quoted\n> - item\n\n---"),
            "<blockquote>\n<p>quoted</p>\n<ul>\n  <li>item</li>\n</ul>\n</blockquote>\n<hr>\n"
        );
    }

    #[test]
    fn renders_tables() {
        let src = "| Name | Count |\n| :--- | ---: |\n| `a|b` | 1 |\n| c \\| d |";
        assert_eq!(
            md(src),
            "<div class=\"table-wrap\"><table>\n<thead>\n\
             <tr><th style=\"text-align: left\">Name</th><th style=\"text-align: right\">Count</th></tr>\n\
             </thead>\n<tbody>\n\
             <tr><td style=\"text-align: left\"><code>a|b</code></td><td style=\"text-align: right\">1</td></tr>\n\
             <tr><td style=\"text-align: left\">c | d</td><td style=\"text-align: right\"></td></tr>\n\
             </tbody>\n</table></div>\n"
        );
        // Without a delimiter row it is just a paragraph.
        assert_eq!(md("| not a table |"), "<p>| not a table |</p>\n");
    }

    #[test]
    fn renders_inline() {
        assert_eq!(
            render_inline("see [the `std` docs](https://doc.rust-lang.org/std/) & <b>", "../"),
            "see <a href=\"https://doc.rust-lang.org/std/\">the <code>std</code> docs</a> &amp; &lt;b&gt;"
        );
        assert_eq!(
            render_inline("[not a link] `open", "../"),
            "[not a link] `open"
        );
        assert_eq!(
            render_inline("`` `tick` `` ``a`b``", ""),
            "<code>`tick`</code> <code>a`b</code>"
        );
    }

    #[test]
    fn renders_emphasis() {
        assert_eq!(
            render_inline("**bold** *em* ~~del~~ 2 * 3 * 4 \\*lit\\*", ""),
            "<strong>bold</strong> <em>em</em> <del>del</del> 2 * 3 * 4 *lit*"
        );
    }

    #[test]
    fn renders_images_and_resolves_urls() {
        assert_eq!(
            render_inline("![a \"cat\"](/images/cat.png \"Cat\")", "../"),
            "<img src=\"../images/cat.png\" alt=\"a &quot;cat&quot;\" title=\"Cat\" loading=\"lazy\">"
        );
        assert_eq!(
            render_inline("[w](https://en.wikipedia.org/wiki/Rust_(lang)) [p](//cdn.example/x)", "../"),
            "<a href=\"https://en.wikipedia.org/wiki/Rust_(lang)\">w</a> <a href=\"//cdn.example/x\">p</a>"
        );
    }

    #[test]
    fn renders_nested_and_task_lists() {
        let src = "- a\n  continued\n  - b\n    1. c\n- [x] done\n- [ ] todo\n\n  more\n\nafter";
        assert_eq!(
            md(src),
            "<ul>\n  <li>a continued\n<ul>\n  <li>b\n<ol>\n  <li>c</li>\n</ol>\n</li>\n</ul>\n</li>\n\
             \x20 <li class=\"task\"><label><input type=\"checkbox\" disabled checked> done</label></li>\n\
             \x20 <li class=\"task\"><label><input type=\"checkbox\" disabled> todo</label>\n<p>more</p>\n</li>\n\
             </ul>\n<p>after</p>\n"
        );
    }

    #[test]
    fn list_items_can_hold_code_blocks() {
        let src = "1. run:\n\n   ```text\n   cargo run\n   ```\n2. done";
        assert_eq!(
            md(src),
            "<ol>\n  <li>run:\n<pre><code class=\"language-text\">cargo run</code></pre>\n</li>\n\
             \x20 <li>done</li>\n</ol>\n"
        );
    }

    #[test]
    fn renders_footnotes_in_reference_order() {
        let src = "B[^b] and A[^a], B again[^b], none[^x].\n\n[^a]: Note *A*.\n[^b]: Note B\n  continued.\n[^unused]: Never shown.";
        let html = md(src);
        assert!(html.starts_with(
            "<p>B<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" role=\"doc-noteref\">1</a></sup> and \
             A<sup class=\"footnote-ref\"><a href=\"#fn-2\" id=\"fnref-2\" role=\"doc-noteref\">2</a></sup>, \
             B again<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1-2\" role=\"doc-noteref\">1</a></sup>, none[^x].</p>"
        ));
        assert!(html.contains("<li id=\"fn-1\">Note B continued. <a href=\"#fnref-1\""));
        assert!(html.contains("<li id=\"fn-2\">Note <em>A</em>. <a href=\"#fnref-2\""));
        assert!(!html.contains("Never shown"));
        assert_eq!(md("[^x](/a)"), "<p><a href=\"../a\">^x</a></p>\n");
    }

    #[test]
    fn builds_table_of_contents() {
        let doc = render_markdown(
            "# Skip?\n## One\n### Sub [link](x)\n## Two\n#### deep\n> ## quoted",
            "",
        );
        // Levels are relative to the highest present (here h1), so h3 is left out.
        assert_eq!(doc.headings.len(), 5);
        let doc = render_markdown("## One\n### Sub [link](x)\n## Two `code`\n#### deep", "");
        assert_eq!(
            render_toc(&doc.headings),
            "<ol class=\"toc\">\n<li><a href=\"#one\">One</a>\n<ol>\n<li><a href=\"#sub-link\">Sub link</a></li>\n</ol>\n</li>\n\
             <li><a href=\"#two-code\">Two <code>code</code></a></li>\n</ol>"
        );
        assert_eq!(render_toc(&render_markdown("## Only", "").headings), "");
    }
}
