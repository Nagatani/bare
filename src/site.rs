//! Site orchestration: discover posts, render them through templates, write `dist/`.

use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::feed;
use crate::images;
use crate::parser::{self, escape_html};
use crate::template::{Context, Template};

/// Where to read sources from and where to write the generated site.
#[derive(Debug, Clone)]
pub struct Config {
    pub content_dir: PathBuf,
    pub out_dir: PathBuf,
    /// Delete the whole output directory before building.
    pub clean: bool,
    /// Include posts marked `draft: true`.
    pub drafts: bool,
    /// Public URL of the site; overrides `base_url` in `site.conf`.
    pub base_url: Option<String>,
}

/// A parsed and rendered post, ready to be placed into templates.
#[derive(Debug)]
pub(crate) struct Post {
    pub slug: String,
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub tags: Vec<String>,
    pub html: String,
}

impl Post {
    /// Path of the rendered page relative to the site root, URL-encoded.
    pub fn path(&self) -> String {
        format!("posts/{}.html", encode_path(&self.slug))
    }
}

/// A standalone page from `content/pages/` (e.g. About), written at the site root.
#[derive(Debug)]
struct Page {
    slug: String,
    title: String,
    description: Option<String>,
    image: Option<String>,
    /// Position in the header navigation; pages without it are not listed.
    nav: Option<i64>,
    html: String,
}

impl Page {
    fn path(&self) -> String {
        format!("{}.html", encode_path(&self.slug))
    }
}

/// Site-wide settings from `content/site.conf`.
#[derive(Debug)]
pub(crate) struct SiteMeta {
    pub title: String,
    pub description: String,
    pub author: String,
    pub lang: String,
    /// Default social preview image.
    pub image: Option<String>,
    /// Absolute public URL, always ending in `/`.
    pub base_url: Option<String>,
}

impl SiteMeta {
    /// Reads `site.conf`. The file is optional; missing keys fall back to defaults.
    fn load(path: &Path, base_url_override: Option<&str>) -> Result<Self, BuildError> {
        let conf = match fs::read_to_string(path) {
            Ok(text) => parser::parse_key_values(&text),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Default::default(),
            Err(err) => return Err(err).at(path),
        };
        let value = |key: &str, default: &str| conf.get(key).unwrap_or(default).to_string();
        let non_empty = |key: &str| conf.get(key).filter(|v| !v.is_empty()).map(str::to_string);

        let base_url = base_url_override
            .map(str::to_string)
            .or_else(|| non_empty("base_url"))
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .map(|url| {
                if url.ends_with('/') {
                    url
                } else {
                    format!("{url}/")
                }
            });

        Ok(Self {
            title: value("title", "bare"),
            description: value("description", ""),
            author: value("author", ""),
            lang: value("lang", "ja"),
            image: non_empty("image"),
            base_url,
        })
    }

    /// The `{{site_*}}` values shared by every template.
    fn context(&self) -> Context {
        Context::new()
            .with("site_title", escape_html(&self.title))
            .with("site_description", escape_html(&self.description))
            .with("site_author", escape_html(&self.author))
            .with("site_lang", escape_html(&self.lang))
            .with(
                "site_url",
                escape_html(self.base_url.as_deref().unwrap_or("")),
            )
    }

    /// Absolute URL for a path relative to the site root, if `base_url` is known.
    pub fn url(&self, path: &str) -> Option<String> {
        let base = self.base_url.as_ref()?;
        Some(format!("{base}{}", path.trim_start_matches('/')))
    }

    /// Absolute URL for an image given as `https://...`, `/images/a.png` or `images/a.png`.
    fn image_url(&self, image: &str) -> Option<String> {
        if image.starts_with("http://") || image.starts_with("https://") {
            Some(image.to_string())
        } else {
            self.url(image)
        }
    }

    /// Path part of `base_url` (e.g. `/bare/`), used where relative links cannot
    /// work because the page may be served at any depth (404.html).
    fn base_path(&self) -> String {
        let path = self
            .base_url
            .as_deref()
            .and_then(|url| url.split_once("://"))
            .and_then(|(_, rest)| rest.find('/').map(|i| &rest[i..]))
            .unwrap_or("/");
        path.to_string()
    }

    /// `<meta>`/`<link>` tags for the `{{meta}}` placeholder: description,
    /// Open Graph, Twitter card, canonical URL and feed discovery.
    fn meta_tags(&self, page: &PageMeta) -> String {
        let mut tags = Vec::new();
        let meta = |attr: &str, key: &str, value: &str| {
            format!("<meta {attr}=\"{key}\" content=\"{}\">", escape_html(value))
        };

        let description = page
            .description
            .filter(|d| !d.is_empty())
            .or(Some(self.description.as_str()).filter(|d| !d.is_empty()));
        if let Some(description) = description {
            tags.push(meta("name", "description", description));
            tags.push(meta("property", "og:description", description));
        }
        tags.push(meta("property", "og:site_name", &self.title));
        tags.push(meta("property", "og:title", page.title));
        let kind = if page.published.is_some() {
            "article"
        } else {
            "website"
        };
        tags.push(meta("property", "og:type", kind));

        if let Some(url) = page.path.and_then(|path| self.url(path)) {
            tags.push(meta("property", "og:url", &url));
            tags.push(format!(
                "<link rel=\"canonical\" href=\"{}\">",
                escape_html(&url)
            ));
        }

        let image = page
            .image
            .or(self.image.as_deref())
            .and_then(|image| self.image_url(image));
        match image {
            Some(image) => {
                tags.push(meta("property", "og:image", &image));
                tags.push(meta("name", "twitter:card", "summary_large_image"));
            }
            None => tags.push(meta("name", "twitter:card", "summary")),
        }

        if let Some(date) = page.published.filter(|date| is_iso_date(date)) {
            tags.push(meta("property", "article:published_time", date));
        }
        if self.base_url.is_some() {
            tags.push(format!(
                "<link rel=\"alternate\" type=\"application/atom+xml\" title=\"{}\" href=\"{}feed.xml\">",
                escape_html(&self.title),
                page.root
            ));
        }
        tags.join("\n  ")
    }
}

/// Per-page inputs for [`SiteMeta::meta_tags`].
struct PageMeta<'a> {
    title: &'a str,
    description: Option<&'a str>,
    /// Path relative to the site root; `None` for pages without a canonical URL.
    path: Option<&'a str>,
    image: Option<&'a str>,
    /// Publication date; marks the page as an article.
    published: Option<&'a str>,
    root: &'a str,
}

/// Summary of a finished build.
#[derive(Debug)]
pub struct BuildReport {
    pub posts: usize,
    /// Standalone pages from `content/pages/`.
    pub pages: usize,
    /// Draft posts left out of the build.
    pub drafts_skipped: usize,
    /// Generated files (pages, feed, sitemap).
    pub written: Vec<PathBuf>,
    /// Number of files copied from `content/static/`.
    pub static_files: usize,
    /// Problems in the sources that did not stop the build.
    pub warnings: Vec<String>,
    /// Informational messages, e.g. features skipped for lack of configuration.
    pub notes: Vec<String>,
    /// URL path the site is served under (e.g. `/bare/`).
    pub base_path: String,
}

/// An I/O failure together with the path that caused it.
#[derive(Debug)]
pub struct BuildError {
    path: PathBuf,
    source: io::Error,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl Error for BuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

trait WithPath<T> {
    fn at(self, path: &Path) -> Result<T, BuildError>;
}

impl<T> WithPath<T> for io::Result<T> {
    fn at(self, path: &Path) -> Result<T, BuildError> {
        self.map_err(|source| BuildError {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Used for `content/pages/` when `templates/page.html` does not exist.
const DEFAULT_PAGE_TEMPLATE: &str =
    "<article class=\"post-body prose\">\n<h1>{{title}}</h1>\n{{content}}\n</article>";

struct Templates {
    layout: Template,
    post: Template,
    index: Template,
    // The rest are optional, so forks with older template sets keep building.
    page: Option<Template>,
    tag: Option<Template>,
    tags: Option<Template>,
    not_found: Option<Template>,
}

impl Templates {
    fn load(dir: &Path) -> Result<Self, BuildError> {
        let load = |name: &str| {
            let path = dir.join(name);
            Template::load(&path).at(&path)
        };
        let optional = |name: &str| match dir.join(name).exists() {
            true => load(name).map(Some),
            false => Ok(None),
        };
        Ok(Self {
            layout: load("layout.html")?,
            post: load("post.html")?,
            index: load("index.html")?,
            page: optional("page.html")?,
            tag: optional("tag.html")?,
            tags: optional("tags.html")?,
            not_found: optional("404.html")?,
        })
    }
}

/// Renders templates with the shared site values, and records placeholders a
/// template uses that the build does not provide (usually typos).
struct Renderer<'a> {
    site: &'a SiteMeta,
    base: Context,
    /// Header navigation entries: (path relative to the root, title).
    nav: Vec<(String, String)>,
    unknown: Vec<String>,
}

impl Renderer<'_> {
    /// Shared values plus `{{root}}`, the relative path from the page to the site root.
    fn context(&self, root: &str) -> Context {
        self.base.clone().with("root", root)
    }

    fn render(&mut self, name: &str, template: &Template, context: &Context) -> String {
        for token in template.unknown_tokens(context) {
            let warning = format!("templates/{name}: unknown placeholder {{{{{token}}}}}");
            if !self.unknown.contains(&warning) {
                self.unknown.push(warning);
            }
        }
        template.render(context)
    }

    /// Wraps a rendered `body` in `layout.html`.
    fn layout(
        &mut self,
        layout: &Template,
        page_title: &str,
        meta: &PageMeta,
        current: Option<&str>,
        body: String,
    ) -> String {
        let context = self
            .context(meta.root)
            .with("title", escape_html(meta.title))
            .with("page_title", escape_html(page_title))
            .with("meta", self.site.meta_tags(meta))
            .with("nav", self.nav_html(meta.root, current))
            .with("content", body);
        self.render("layout.html", layout, &context)
    }

    fn nav_html(&self, root: &str, current: Option<&str>) -> String {
        if self.nav.is_empty() {
            return String::new();
        }
        let links: Vec<String> = self
            .nav
            .iter()
            .map(|(path, title)| {
                let aria = if current == Some(path.as_str()) {
                    " aria-current=\"page\""
                } else {
                    ""
                };
                format!(
                    "<a href=\"{root}{}\"{aria}>{}</a>",
                    escape_html(path),
                    escape_html(title)
                )
            })
            .collect();
        format!("<nav class=\"site-nav\">{}</nav>", links.join(""))
    }
}

/// Builds the whole site described by `config`.
pub fn build(config: &Config) -> Result<BuildReport, BuildError> {
    let templates = Templates::load(&config.content_dir.join("templates"))?;
    let site = SiteMeta::load(
        &config.content_dir.join("site.conf"),
        config.base_url.as_deref(),
    )?;
    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    let (mut posts, post_drafts) = load_posts(
        &config.content_dir.join("posts"),
        config.drafts,
        &mut warnings,
    )?;
    // Newest first; ISO dates (YYYY-MM-DD) sort correctly as strings.
    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));
    let (pages, page_drafts) = load_pages(
        &config.content_dir.join("pages"),
        config.drafts,
        &mut warnings,
    )?;

    if config.clean && config.out_dir.exists() {
        ensure_safe_to_clean(config)?;
        fs::remove_dir_all(&config.out_dir).at(&config.out_dir)?;
    }

    // Without `clean`, only generated directories are cleared, so stale pages from
    // deleted posts or tags disappear without touching anything else.
    for dir in ["posts", "tags"] {
        let dir = config.out_dir.join(dir);
        if dir.exists() {
            fs::remove_dir_all(&dir).at(&dir)?;
        }
    }
    let posts_out = config.out_dir.join("posts");
    fs::create_dir_all(&posts_out).at(&posts_out)?;

    // Static files go in first, so a generated page wins over a same-named file.
    let static_files = copy_dir(&config.content_dir.join("static"), &config.out_dir)?;

    let mut nav: Vec<&Page> = pages.iter().filter(|page| page.nav.is_some()).collect();
    nav.sort_by(|a, b| a.nav.cmp(&b.nav).then_with(|| a.title.cmp(&b.title)));
    let mut r = Renderer {
        site: &site,
        base: site.context(),
        nav: nav
            .iter()
            .map(|page| (page.path(), page.title.clone()))
            .collect(),
        unknown: Vec::new(),
    };
    let link_tags = templates.tag.is_some();

    let mut written = Vec::new();
    let mut write = |path: PathBuf, contents: String| -> Result<(), BuildError> {
        fs::write(&path, contents).at(&path)?;
        written.push(path);
        Ok(())
    };
    let mut sitemap = vec![(String::new(), posts.first().map(|p| p.date.clone()))];

    for post in &posts {
        let root = "../";
        let path = post.path();
        let context = r
            .context(root)
            .with("title", escape_html(&post.title))
            .with("date", escape_html(&post.date))
            .with(
                "description",
                escape_html(post.description.as_deref().unwrap_or("")),
            )
            .with("tags", render_tags(&post.tags, link_tags.then_some(root)))
            .with(
                "content",
                images::add_dimensions(&post.html, &config.out_dir, &posts_out),
            );
        let body = r.render("post.html", &templates.post, &context);
        let meta = PageMeta {
            title: &post.title,
            description: post.description.as_deref(),
            path: Some(&path),
            image: post.image.as_deref(),
            published: Some(&post.date),
            root,
        };
        let page_title = format!("{} · {}", post.title, site.title);
        let html = r.layout(&templates.layout, &page_title, &meta, None, body);
        write(posts_out.join(format!("{}.html", post.slug)), html)?;
        sitemap.push((path, Some(post.date.clone())));
    }

    // Without page.html (older template sets), use a minimal built-in one; the
    // post template would carry post-only links such as `../index.html`.
    let fallback = Template::new(DEFAULT_PAGE_TEMPLATE);
    let (page_name, page_template) = match &templates.page {
        Some(template) => ("page.html", template),
        None => ("built-in page template", &fallback),
    };
    for page in &pages {
        let root = "";
        let path = page.path();
        let context = r
            .context(root)
            .with("title", escape_html(&page.title))
            .with("date", "")
            .with("tags", "")
            .with(
                "description",
                escape_html(page.description.as_deref().unwrap_or("")),
            )
            .with(
                "content",
                images::add_dimensions(&page.html, &config.out_dir, &config.out_dir),
            );
        let body = r.render(page_name, page_template, &context);
        let meta = PageMeta {
            title: &page.title,
            description: page.description.as_deref(),
            path: Some(&path),
            image: page.image.as_deref(),
            published: None,
            root,
        };
        let page_title = format!("{} · {}", page.title, site.title);
        let html = r.layout(&templates.layout, &page_title, &meta, Some(&path), body);
        write(config.out_dir.join(format!("{}.html", page.slug)), html)?;
        sitemap.push((path, None));
    }

    let all: Vec<&Post> = posts.iter().collect();
    let context = r
        .context("")
        .with("post_list", render_post_list(&all, "", link_tags))
        .with("post_count", posts.len().to_string());
    let body = r.render("index.html", &templates.index, &context);
    let meta = PageMeta {
        title: &site.title,
        description: None,
        path: Some(""),
        image: None,
        published: None,
        root: "",
    };
    let html = r.layout(&templates.layout, &site.title, &meta, Some(""), body);
    write(config.out_dir.join("index.html"), html)?;

    if let Some(tag_template) = &templates.tag {
        let groups = group_by_tag(&posts);
        let tags_out = config.out_dir.join("tags");
        fs::create_dir_all(&tags_out).at(&tags_out)?;
        let root = "../";
        for group in &groups {
            let path = group.path();
            let context = r
                .context(root)
                .with("tag", escape_html(&group.name))
                .with("post_list", render_post_list(&group.posts, root, true))
                .with("post_count", group.posts.len().to_string());
            let body = r.render("tag.html", tag_template, &context);
            let title = format!("#{}", group.name);
            let meta = PageMeta {
                title: &title,
                description: None,
                path: Some(&path),
                image: None,
                published: None,
                root,
            };
            let page_title = format!("{title} · {}", site.title);
            let html = r.layout(&templates.layout, &page_title, &meta, None, body);
            write(tags_out.join(format!("{}.html", group.file)), html)?;
            sitemap.push((path, group.posts.first().map(|p| p.date.clone())));
        }

        if let Some(tags_template) = &templates.tags {
            let context = r
                .context(root)
                .with("tag_list", render_tag_list(&groups, root))
                .with("tag_count", groups.len().to_string());
            let body = r.render("tags.html", tags_template, &context);
            let meta = PageMeta {
                title: "Tags",
                description: None,
                path: Some("tags/"),
                image: None,
                published: None,
                root,
            };
            let page_title = format!("Tags · {}", site.title);
            let html = r.layout(&templates.layout, &page_title, &meta, None, body);
            write(tags_out.join("index.html"), html)?;
            sitemap.push(("tags/".to_string(), None));
        }
    }

    if let Some(not_found) = &templates.not_found {
        // GitHub Pages serves 404.html for a missing URL at any depth, so links
        // must be root-relative (e.g. `/bare/`) rather than `../`.
        let root = site.base_path();
        let context = r.context(&root);
        let body = r.render("404.html", not_found, &context);
        let meta = PageMeta {
            title: "404 Not Found",
            description: None,
            path: None,
            image: None,
            published: None,
            root: &root,
        };
        let page_title = format!("404 Not Found · {}", site.title);
        let html = r.layout(&templates.layout, &page_title, &meta, None, body);
        write(config.out_dir.join("404.html"), html)?;
    }

    if site.base_url.is_some() {
        write(config.out_dir.join("feed.xml"), feed::atom(&site, &posts))?;
        write(
            config.out_dir.join("sitemap.xml"),
            feed::sitemap(&site, &sitemap),
        )?;
    } else {
        notes.push(
            "base_url is not set: skipped feed.xml, sitemap.xml and absolute URLs \
             (pass --base-url or set base_url in site.conf; CI sets it automatically)"
                .to_string(),
        );
    }
    warnings.append(&mut r.unknown);

    Ok(BuildReport {
        posts: posts.len(),
        pages: pages.len(),
        drafts_skipped: post_drafts + page_drafts,
        written,
        static_files,
        warnings,
        notes,
        base_path: site.base_path(),
    })
}

/// Refuses to wipe an output directory that contains the working directory or
/// the content sources (e.g. `--out .` or `--out ..`).
fn ensure_safe_to_clean(config: &Config) -> Result<(), BuildError> {
    let out = fs::canonicalize(&config.out_dir).at(&config.out_dir)?;
    let content = fs::canonicalize(&config.content_dir).at(&config.content_dir)?;
    let cwd = env::current_dir()
        .and_then(fs::canonicalize)
        .at(Path::new("."))?;

    if out.parent().is_none() || cwd.starts_with(&out) || content.starts_with(&out) {
        return Err(BuildError {
            path: config.out_dir.clone(),
            source: io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to clean: it contains the working directory or the content sources",
            ),
        });
    }
    Ok(())
}

/// Recursively copies the contents of `from` into `to`, returning the number of
/// files copied. A missing `from` directory copies nothing.
fn copy_dir(from: &Path, to: &Path) -> Result<usize, BuildError> {
    if !from.is_dir() {
        return Ok(0);
    }
    fs::create_dir_all(to).at(to)?;
    let mut count = 0;
    for entry in fs::read_dir(from).at(from)? {
        let entry = entry.at(from)?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type().at(&source)?.is_dir() {
            count += copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target).at(&source)?;
            count += 1;
        }
    }
    Ok(count)
}

/// A Markdown file read from disk, with its file stem as the slug.
struct Source {
    path: PathBuf,
    slug: String,
    doc: parser::Document,
}

/// Reads every `*.md` file directly inside `dir`, leaving out drafts unless
/// `include_drafts`. Returns the sources and the number of drafts left out.
/// A missing directory simply has no files (git does not keep empty directories).
fn read_sources(dir: &Path, include_drafts: bool) -> Result<(Vec<Source>, usize), BuildError> {
    if !dir.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).at(dir)? {
        let path = entry.at(dir)?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    let mut drafts = 0;
    for path in paths {
        let text = fs::read_to_string(&path).at(&path)?;
        let doc = parser::parse_document(&text);
        let is_draft = doc
            .frontmatter
            .get("draft")
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "yes"));
        if is_draft && !include_drafts {
            drafts += 1;
            continue;
        }
        let slug = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        sources.push(Source { path, slug, doc });
    }
    Ok((sources, drafts))
}

/// Value of an optional, non-empty frontmatter key.
fn optional(meta: &parser::Frontmatter, key: &str) -> Option<String> {
    meta.get(key).filter(|v| !v.is_empty()).map(str::to_string)
}

fn load_posts(
    dir: &Path,
    include_drafts: bool,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Post>, usize), BuildError> {
    let (sources, drafts) = read_sources(dir, include_drafts)?;
    let posts = sources
        .into_iter()
        .map(|Source { path, slug, doc }| {
            let meta = &doc.frontmatter;
            let date = meta.get("date").unwrap_or_default().to_string();
            if !is_iso_date(&date) {
                warnings.push(format!(
                    "{}: date {date:?} is not YYYY-MM-DD; post order and feed dates may be wrong",
                    path.display()
                ));
            }
            Post {
                title: meta.get("title").unwrap_or(&slug).to_string(),
                date,
                description: optional(meta, "description"),
                image: optional(meta, "image"),
                tags: meta.tags(),
                // Posts live in posts/, one level below the site root.
                html: parser::markdown_to_html(&doc.body, "../"),
                slug,
            }
        })
        .collect();
    Ok((posts, drafts))
}

fn load_pages(
    dir: &Path,
    include_drafts: bool,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Page>, usize), BuildError> {
    let (sources, drafts) = read_sources(dir, include_drafts)?;
    let mut pages = Vec::with_capacity(sources.len());
    for Source { path, slug, doc } in sources {
        if matches!(slug.as_str(), "index" | "404") {
            warnings.push(format!(
                "{}: skipped, {slug}.html is generated by bare",
                path.display()
            ));
            continue;
        }
        let meta = &doc.frontmatter;
        pages.push(Page {
            title: meta.get("title").unwrap_or(&slug).to_string(),
            description: optional(meta, "description"),
            image: optional(meta, "image"),
            nav: meta.get("nav").and_then(|v| v.trim().parse().ok()),
            // Pages are written at the site root.
            html: parser::markdown_to_html(&doc.body, ""),
            slug,
        });
    }
    Ok((pages, drafts))
}

/// True for dates shaped like `2026-11-01`.
pub(crate) fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Percent-encodes everything except unreserved URL characters, so file names
/// with spaces or non-ASCII characters still produce valid links.
pub(crate) fn encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Posts sharing one tag (compared case-insensitively).
struct TagGroup<'a> {
    name: String,
    file: String,
    posts: Vec<&'a Post>,
}

impl TagGroup<'_> {
    fn path(&self) -> String {
        format!("tags/{}.html", encode_path(&self.file))
    }
}

/// Groups posts by tag, keeping post order (newest first) and sorting tags by name.
fn group_by_tag(posts: &[Post]) -> Vec<TagGroup<'_>> {
    let mut groups: Vec<TagGroup> = Vec::new();
    for post in posts {
        for tag in &post.tags {
            let file = tag_file(tag);
            match groups.iter_mut().find(|group| group.file == file) {
                Some(group) => {
                    if !group.posts.iter().any(|p| std::ptr::eq(*p, post)) {
                        group.posts.push(post);
                    }
                }
                None => groups.push(TagGroup {
                    name: tag.clone(),
                    file,
                    posts: vec![post],
                }),
            }
        }
    }
    groups.sort_by_key(|group| group.name.to_lowercase());
    groups
}

/// File name for a tag: lowercase, characters unsafe in paths replaced by `-`.
fn tag_file(tag: &str) -> String {
    tag.trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Renders tags, as links to their tag pages when `root` is given.
fn render_tags(tags: &[String], root: Option<&str>) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let items: Vec<String> = tags
        .iter()
        .map(|tag| match root {
            Some(root) => format!(
                "<li class=\"tag\"><a href=\"{root}tags/{}.html\">{}</a></li>",
                escape_html(&encode_path(&tag_file(tag))),
                escape_html(tag)
            ),
            None => format!("<li class=\"tag\">{}</li>", escape_html(tag)),
        })
        .collect();
    format!("<ul class=\"tags\">{}</ul>", items.join(""))
}

/// Renders `<li>` items for a post list on a page `root` away from the site root.
fn render_post_list(posts: &[&Post], root: &str, link_tags: bool) -> String {
    let mut out = String::new();
    for post in posts {
        let description = post
            .description
            .as_deref()
            .map(|d| format!("<p class=\"post-desc\">{}</p>", escape_html(d)))
            .unwrap_or_default();
        out.push_str(&format!(
            "<li class=\"post-item\">\
             <time datetime=\"{date}\">{date}</time>\
             <a href=\"{root}{href}\">{title}</a>\
             {description}{tags}</li>\n",
            date = escape_html(&post.date),
            href = escape_html(&post.path()),
            title = escape_html(&post.title),
            tags = render_tags(&post.tags, link_tags.then_some(root)),
        ));
    }
    out
}

/// Renders `<li>` items for the tag index, most used tags first.
fn render_tag_list(groups: &[TagGroup], root: &str) -> String {
    let mut sorted: Vec<&TagGroup> = groups.iter().collect();
    sorted.sort_by(|a, b| b.posts.len().cmp(&a.posts.len()));
    let mut out = String::new();
    for group in sorted {
        out.push_str(&format!(
            "<li><a class=\"tag\" href=\"{root}{}\">{}</a><span class=\"tag-count\">{}</span></li>\n",
            escape_html(&group.path()),
            escape_html(&group.name),
            group.posts.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(base_url: Option<&str>) -> SiteMeta {
        SiteMeta {
            title: "Blog".into(),
            description: "Notes".into(),
            author: String::new(),
            lang: "ja".into(),
            image: None,
            base_url: base_url.map(str::to_string),
        }
    }

    #[test]
    fn validates_iso_dates() {
        assert!(is_iso_date("2026-11-01"));
        assert!(!is_iso_date("2026/11/01"));
        assert!(!is_iso_date("2026-1-1"));
        assert!(!is_iso_date(""));
    }

    #[test]
    fn encodes_paths() {
        assert_eq!(encode_path("hello-world"), "hello-world");
        assert_eq!(encode_path("a b"), "a%20b");
        assert_eq!(encode_path("日"), "%E6%97%A5");
    }

    #[test]
    fn resolves_urls_against_base() {
        let s = site(Some("https://u.github.io/bare/"));
        assert_eq!(
            s.url("posts/a.html").as_deref(),
            Some("https://u.github.io/bare/posts/a.html")
        );
        assert_eq!(
            s.image_url("/images/a.png").as_deref(),
            Some("https://u.github.io/bare/images/a.png")
        );
        assert_eq!(s.base_path(), "/bare/");
        assert_eq!(site(Some("https://example.com/")).base_path(), "/");
        assert_eq!(site(None).url("x"), None);
    }

    #[test]
    fn meta_tags_skip_absolute_urls_without_base() {
        let page = PageMeta {
            title: "Post",
            description: None,
            path: Some("posts/a.html"),
            image: Some("/images/a.png"),
            published: Some("2026-11-01"),
            root: "../",
        };
        let without = site(None).meta_tags(&page);
        assert!(without.contains("<meta name=\"description\" content=\"Notes\">"));
        assert!(
            !without.contains("og:url")
                && !without.contains("og:image")
                && !without.contains("feed.xml")
        );

        let with = site(Some("https://u.github.io/bare/")).meta_tags(&page);
        assert!(with.contains(
            "<meta property=\"og:url\" content=\"https://u.github.io/bare/posts/a.html\">"
        ));
        assert!(with.contains(
            "<meta property=\"og:image\" content=\"https://u.github.io/bare/images/a.png\">"
        ));
        assert!(with.contains("href=\"../feed.xml\""));
        assert!(with.contains("article:published_time"));
    }
}
