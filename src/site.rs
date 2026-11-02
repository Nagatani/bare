//! Site orchestration: discover posts, render them through templates, write `dist/`.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::assets::Fingerprints;
use crate::css;
use crate::feed;
use crate::images;
use crate::og;
use crate::parser::{self, escape_html};
use crate::template::{Args, Context, Kind, Spec, Template};
use crate::text;

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
    /// Generate OGP images with Chrome (see `og`). Off for `serve`, where
    /// previews are rebuilt on every save and nobody sees the images.
    pub og_images: bool,
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
    /// Headings for the table of contents (`{{toc}}`).
    headings: Vec<parser::Heading>,
    /// Start of the first paragraph, used when there is no description.
    excerpt: String,
    reading_minutes: usize,
}

impl Post {
    /// Path of the rendered page relative to the site root, URL-encoded.
    pub fn path(&self) -> String {
        format!("posts/{}.html", encode_path(&self.slug))
    }

    /// The description, or else an excerpt of the post.
    pub fn summary(&self) -> &str {
        self.description.as_deref().unwrap_or(&self.excerpt)
    }

    fn year(&self) -> &str {
        self.date.get(..4).unwrap_or("")
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
    headings: Vec<parser::Heading>,
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
    /// Posts per page of the post list; `None` puts all posts on one page.
    per_page: Option<usize>,
    /// Names of the generated list pages, used in `<title>` and OGP. They live
    /// in site.conf rather than templates because the engine builds the title.
    archive_title: String,
    tags_title: String,
    not_found_title: String,
    /// `og_image` in site.conf: `off` turns generated OGP images off.
    og_image: bool,
    /// `theme` in site.conf: a directory name under `content/themes/`.
    /// `None` means the default theme (or the old layout without themes/).
    theme: Option<String>,
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
            per_page: conf
                .get("per_page")
                .and_then(|v| v.trim().parse().ok())
                .filter(|&n| n > 0),
            // non_empty, not value: an empty key falls back to the default
            // instead of producing a blank page title.
            archive_title: non_empty("archive_title").unwrap_or_else(|| "Archive".into()),
            tags_title: non_empty("tags_title").unwrap_or_else(|| "Tags".into()),
            not_found_title: non_empty("not_found_title").unwrap_or_else(|| "404 Not Found".into()),
            og_image: !matches!(conf.get("og_image").map(str::trim), Some("off")),
            theme: non_empty("theme").map(|t| t.trim().to_string()),
        })
    }

    /// The `{{site_*}}` values shared by every template.
    fn context(&self) -> Context<'static> {
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
    /// Number of static files copied (site static plus theme files).
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

/// Directories that templates and static files are read from, highest
/// priority first. Lookups take the first directory that has a given file.
///
/// With `content/themes/` (the current layout):
/// - templates: `content/templates/` (old layout, only if left over), the
///   selected theme's `templates/`, then `themes/default/templates/`
/// - static: `content/static/`, the selected theme's `static/`, then
///   `themes/default/static/`
///
/// Without `content/themes/` (forks that predate themes), everything is read
/// from `content/templates/` and `content/static/` exactly as before.
struct Layers {
    templates: Vec<PathBuf>,
    statics: Vec<PathBuf>,
}

impl Layers {
    fn resolve(
        content_dir: &Path,
        theme: Option<&str>,
        warnings: &mut Vec<String>,
    ) -> Result<Self, BuildError> {
        let themes_dir = content_dir.join("themes");
        let legacy_templates = content_dir.join("templates");
        let site_static = content_dir.join("static");
        let config_error = |path: &Path, message: String| {
            Err(io::Error::new(io::ErrorKind::NotFound, message)).at(path)
        };

        if !themes_dir.is_dir() {
            if let Some(name) = theme {
                return config_error(
                    &themes_dir,
                    format!("site.conf sets theme: {name}, but this directory does not exist"),
                );
            }
            return Ok(Self {
                templates: vec![legacy_templates],
                statics: vec![site_static],
            });
        }

        let name = theme.unwrap_or(DEFAULT_THEME);
        // The name becomes a path component; anything that could leave
        // content/themes/ (separators, `..`) is rejected outright.
        if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
            return config_error(
                &content_dir.join("site.conf"),
                format!("invalid theme name {name:?}: use a directory name under themes/"),
            );
        }
        let selected = themes_dir.join(name);
        if !selected.is_dir() {
            return config_error(&selected, format!("theme {name:?} not found"));
        }
        let mut chain = vec![selected];
        let default = themes_dir.join(DEFAULT_THEME);
        if name != DEFAULT_THEME && default.is_dir() {
            chain.push(default);
        }

        let mut templates = Vec::new();
        // Usually left behind when a fork that edited content/templates/ merges
        // the switch to themes. It still wins, so the site keeps its look, but
        // the files should move into a theme of their own.
        if legacy_templates.is_dir() {
            warnings.push(format!(
                "{}/ overrides the theme; move these files to themes/<your theme>/templates/ \
                 and set theme: in site.conf (see CHANGELOG.md)",
                legacy_templates.display()
            ));
            templates.push(legacy_templates);
        }
        templates.extend(chain.iter().map(|dir| dir.join("templates")));
        let mut statics = vec![site_static];
        statics.extend(chain.iter().map(|dir| dir.join("static")));
        Ok(Self { templates, statics })
    }
}

/// The theme used when site.conf has no `theme`, and the fallback for files a
/// theme does not provide.
const DEFAULT_THEME: &str = "default";

struct Templates {
    layout: Template,
    post: Template,
    index: Template,
    // The rest are optional, so forks with older template sets keep building.
    page: Option<Template>,
    archive: Option<Template>,
    tag: Option<Template>,
    tags: Option<Template>,
    not_found: Option<Template>,
    /// The OGP image card; without it no images are generated.
    og: Option<Template>,
}

impl Templates {
    /// Loads each template from the first of `dirs` (highest priority first)
    /// that has it, so a theme can replace single files and inherit the rest.
    fn load(dirs: &[PathBuf]) -> Result<Self, BuildError> {
        let find = |name: &str| dirs.iter().map(|dir| dir.join(name)).find(|p| p.is_file());
        let load_from = |path: PathBuf| Template::load(&path).at(&path);
        let load = |name: &str| match find(name) {
            Some(path) => load_from(path),
            None => {
                let searched: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "required template not found (searched {})",
                        searched.join(", ")
                    ),
                ))
                .at(Path::new(name))
            }
        };
        let optional = |name: &str| find(name).map(load_from).transpose();
        Ok(Self {
            layout: load("layout.html")?,
            post: load("post.html")?,
            index: load("index.html")?,
            page: optional("page.html")?,
            archive: optional("archive.html")?,
            tag: optional("tag.html")?,
            tags: optional("tags.html")?,
            not_found: optional("404.html")?,
            og: optional("og.html")?,
        })
    }
}

/// Renders templates with the shared site values, and records placeholders a
/// template uses that the build does not provide (usually typos).
struct Renderer<'a> {
    site: &'a SiteMeta,
    /// Site-wide values. `'static` because they own their strings; per-page
    /// contexts start from a clone and add values that borrow posts.
    base: Context<'static>,
    /// Header navigation entries: (path relative to the root, title).
    nav: Vec<(String, String)>,
    unknown: Vec<String>,
}

impl Renderer<'_> {
    /// Shared values plus `{{root}}`, the relative path from the page to the site root.
    ///
    /// On pages at the site root the relative path is empty, but `{{root}}`
    /// becomes `./` there: templates link home with `href="{{root}}"`, and an
    /// empty `href` would point at the current page instead (e.g. archive.html).
    /// `./css/style.css` and `css/style.css` mean the same, so prefixes still work.
    fn context(&self, root: &str) -> Context<'static> {
        let root = if root.is_empty() { "./" } else { root };
        self.base.clone().with("root", root)
    }

    fn render(&mut self, template: &Template, context: &Context<'_>) -> String {
        for problem in template.problems(context) {
            // The origin is the file's path, so a warning points at the theme
            // file that actually supplied the template.
            let warning = format!("{}: {problem}", template.origin());
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
        // The closure gets its own copy: borrowing `self.nav` would keep `self`
        // borrowed while `self.render` below needs it mutably.
        let nav = self.nav.clone();
        let context = self
            .context(meta.root)
            .with("title", escape_html(meta.title))
            .with("page_title", escape_html(page_title))
            .with("meta", self.site.meta_tags(meta))
            .with_markup("nav", NAV_ARGS, move |args| {
                render_nav(&nav, meta.root, current, args)
            })
            .with("content", body);
        self.render(layout, &context)
    }
}

/// Builds the whole site described by `config`.
pub fn build(config: &Config) -> Result<BuildReport, BuildError> {
    let site = SiteMeta::load(
        &config.content_dir.join("site.conf"),
        config.base_url.as_deref(),
    )?;
    let mut warnings = Vec::new();
    let layers = Layers::resolve(&config.content_dir, site.theme.as_deref(), &mut warnings)?;
    let templates = Templates::load(&layers.templates)?;
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
    for dir in ["posts", "tags", "page", "og"] {
        let dir = config.out_dir.join(dir);
        if dir.exists() {
            fs::remove_dir_all(&dir).at(&dir)?;
        }
    }
    let posts_out = config.out_dir.join("posts");
    fs::create_dir_all(&posts_out).at(&posts_out)?;

    // Static files go in first, so a generated page wins over a same-named file.
    let static_files = copy_static(&layers.statics, &config.out_dir, &mut warnings)?;

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
    let groups = group_by_tag(&posts);
    let og_pages = og_requests(&site, &templates, &posts, &pages, &groups);
    let og_done = og_images(
        config,
        &site,
        &templates,
        &og_pages,
        &mut r,
        &mut written,
        (&mut warnings, &mut notes),
    );
    let base_path = site.base_path();
    let mut fingerprints = Fingerprints::new(&config.out_dir, &base_path);
    let mut write = |path: PathBuf, contents: String| -> Result<(), BuildError> {
        let contents = match (path.extension(), path.parent()) {
            (Some(ext), Some(dir)) if ext == "html" => fingerprints.apply(&contents, dir),
            _ => contents,
        };
        fs::write(&path, contents).at(&path)?;
        written.push(path);
        Ok(())
    };
    let mut sitemap = vec![(String::new(), posts.first().map(|p| p.date.clone()))];

    for post in &posts {
        let root = "../";
        let path = post.path();
        let generated = og_url(&og_done, &format!("posts/{}.png", post.slug));
        let context = r
            .context(root)
            .with("title", escape_html(&post.title))
            .with("date", escape_html(&post.date))
            .with(
                "description",
                escape_html(post.description.as_deref().unwrap_or("")),
            )
            .with_markup("tags", TAGS_ARGS, move |args| {
                render_tags(&post.tags, link_tags.then_some(root), args)
            })
            .with_markup("toc", TOC_ARGS, |args| {
                parser::render_toc(&post.headings, args.text("class", "toc"))
            })
            .with("reading_minutes", post.reading_minutes.to_string())
            .with(
                "content",
                images::add_dimensions(&post.html, &config.out_dir, &posts_out),
            );
        let body = r.render(&templates.post, &context);
        let meta = PageMeta {
            title: &post.title,
            description: Some(post.summary()),
            path: Some(&path),
            // The post's own image wins; then its generated card; then the
            // site-wide image (applied in meta_tags).
            image: post.image.as_deref().or(generated.as_deref()),
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
    let fallback = Template::new(DEFAULT_PAGE_TEMPLATE).named("built-in page template");
    let page_template = templates.page.as_ref().unwrap_or(&fallback);
    for page in &pages {
        let root = "";
        let path = page.path();
        let generated = og_url(&og_done, &format!("{}.png", page.slug));
        let context = r
            .context(root)
            .with("title", escape_html(&page.title))
            .with("date", "")
            // Pages have no tags. Still a markup placeholder, so `{{tags class="…"}}`
            // copied from post.html does not warn about arguments.
            .with_markup("tags", TAGS_ARGS, |_| String::new())
            .with_markup("toc", TOC_ARGS, |args| {
                parser::render_toc(&page.headings, args.text("class", "toc"))
            })
            .with(
                "description",
                escape_html(page.description.as_deref().unwrap_or("")),
            )
            .with(
                "content",
                images::add_dimensions(&page.html, &config.out_dir, &config.out_dir),
            );
        let body = r.render(page_template, &context);
        let meta = PageMeta {
            title: &page.title,
            description: page.description.as_deref(),
            path: Some(&path),
            image: page.image.as_deref().or(generated.as_deref()),
            published: None,
            root,
        };
        let page_title = format!("{} · {}", page.title, site.title);
        let html = r.layout(&templates.layout, &page_title, &meta, Some(&path), body);
        write(config.out_dir.join(format!("{}.html", page.slug)), html)?;
        sitemap.push((path, None));
    }

    // The post list: index.html, then page/2.html, page/3.html, ... with `per_page`.
    let all: Vec<&Post> = posts.iter().collect();
    let chunks: Vec<&[&Post]> = match site.per_page {
        Some(n) if !all.is_empty() => all.chunks(n).collect(),
        _ => vec![&all[..]],
    };
    let page_count = chunks.len();
    if page_count > 1 && !templates.index.uses("pagination") {
        warnings.push(format!(
            "per_page is set but {} has no {{{{pagination}}}}: \
             posts after the first page are only reachable from tags and archive.html",
            templates.index.origin()
        ));
    }
    if page_count > 1 {
        let dir = config.out_dir.join("page");
        fs::create_dir_all(&dir).at(&dir)?;
    }
    // Every page of the post list shares the top page's image.
    let index_image = og_url(&og_done, "index.png");
    for (index, chunk) in chunks.iter().enumerate() {
        let number = index + 1;
        let (root, path) = match number {
            1 => ("", String::new()),
            n => ("../", format!("page/{n}.html")),
        };
        let context = r
            .context(root)
            .with_markup("post_list", POST_LIST_ARGS, |args| {
                render_post_list(chunk, root, link_tags, args)
            })
            .with("post_count", posts.len().to_string())
            .with_markup("pagination", PAGINATION_ARGS, |args| {
                render_pagination(number, page_count, root, args)
            })
            .with("page_number", number.to_string())
            .with("page_count", page_count.to_string());
        let body = r.render(&templates.index, &context);
        let title = match number {
            1 => site.title.clone(),
            n => format!("{} ({n}/{page_count})", site.title),
        };
        let meta = PageMeta {
            title: &title,
            description: None,
            path: Some(&path),
            image: index_image.as_deref(),
            published: None,
            root,
        };
        let current = (number == 1).then_some("");
        let html = r.layout(&templates.layout, &title, &meta, current, body);
        write(
            config.out_dir.join(if number == 1 {
                "index.html".to_string()
            } else {
                path.clone()
            }),
            html,
        )?;
        if number > 1 {
            sitemap.push((path, chunk.first().map(|p| p.date.clone())));
        }
    }

    if let Some(archive) = &templates.archive {
        let context = r
            .context("")
            .with_markup("archive", ARCHIVE_ARGS, |args| {
                render_archive(&posts, link_tags, args)
            })
            .with("post_count", posts.len().to_string());
        let body = r.render(archive, &context);
        let image = og_url(&og_done, "archive.png");
        let meta = PageMeta {
            title: &site.archive_title,
            description: None,
            path: Some("archive.html"),
            image: image.as_deref(),
            published: None,
            root: "",
        };
        let page_title = format!("{} · {}", site.archive_title, site.title);
        let html = r.layout(&templates.layout, &page_title, &meta, None, body);
        write(config.out_dir.join("archive.html"), html)?;
        sitemap.push((
            "archive.html".to_string(),
            posts.first().map(|p| p.date.clone()),
        ));
    }

    if let Some(tag_template) = &templates.tag {
        let tags_out = config.out_dir.join("tags");
        fs::create_dir_all(&tags_out).at(&tags_out)?;
        let root = "../";
        for group in &groups {
            let path = group.path();
            let context = r
                .context(root)
                .with("tag", escape_html(&group.name))
                .with_markup("post_list", POST_LIST_ARGS, |args| {
                    render_post_list(&group.posts, root, true, args)
                })
                .with("post_count", group.posts.len().to_string());
            let body = r.render(tag_template, &context);
            let title = format!("#{}", group.name);
            let image = og_url(&og_done, &format!("tags/{}.png", group.file));
            let meta = PageMeta {
                title: &title,
                description: None,
                path: Some(&path),
                image: image.as_deref(),
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
                .with_markup("tag_list", TAG_LIST_ARGS, |args| {
                    render_tag_list(&groups, root, args)
                })
                .with("tag_count", groups.len().to_string());
            let body = r.render(tags_template, &context);
            let image = og_url(&og_done, "tags/index.png");
            let meta = PageMeta {
                title: &site.tags_title,
                description: None,
                path: Some("tags/"),
                image: image.as_deref(),
                published: None,
                root,
            };
            let page_title = format!("{} · {}", site.tags_title, site.title);
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
        let body = r.render(not_found, &context);
        let meta = PageMeta {
            title: &site.not_found_title,
            description: None,
            path: None,
            image: None,
            published: None,
            root: &root,
        };
        let page_title = format!("{} · {}", site.not_found_title, site.title);
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
        base_path,
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

/// The pages that should get a generated OGP image, with what goes on each card.
///
/// Posts and static pages get one unless their frontmatter sets `image`. List
/// pages (top, archive, tags) get one only when site.conf has no `image`: a
/// site-wide image was chosen on purpose and stays their image, as before
/// these were generated. The titles match the pages' `<title>`s.
fn og_requests(
    site: &SiteMeta,
    templates: &Templates,
    posts: &[Post],
    pages: &[Page],
    groups: &[TagGroup],
) -> Vec<OgPage> {
    let card = |file: String, kind, title: &str, date: &str, description: &str| OgPage {
        file,
        kind,
        title: title.to_string(),
        date: date.to_string(),
        description: description.to_string(),
    };
    let mut out = Vec::new();
    for post in posts.iter().filter(|post| post.image.is_none()) {
        let description = post.description.as_deref().unwrap_or("");
        out.push(card(
            format!("posts/{}.png", post.slug),
            "post",
            &post.title,
            &post.date,
            description,
        ));
    }
    for page in pages.iter().filter(|page| page.image.is_none()) {
        let description = page.description.as_deref().unwrap_or("");
        out.push(card(
            format!("{}.png", page.slug),
            "page",
            &page.title,
            "",
            description,
        ));
    }
    if site.image.is_some() {
        return out;
    }
    out.push(card(
        "index.png".into(),
        "index",
        &site.title,
        "",
        &site.description,
    ));
    if templates.archive.is_some() {
        out.push(card(
            "archive.png".into(),
            "archive",
            &site.archive_title,
            "",
            "",
        ));
    }
    if templates.tag.is_some() {
        for group in groups {
            let title = format!("#{}", group.name);
            out.push(card(
                format!("tags/{}.png", group.file),
                "tag",
                &title,
                "",
                "",
            ));
        }
        if templates.tags.is_some() {
            out.push(card(
                "tags/index.png".into(),
                "tags",
                &site.tags_title,
                "",
                "",
            ));
        }
    }
    out
}

/// A page that may get a generated OGP image.
struct OgPage {
    /// Where the image goes under `og/`, mirroring the page's own path
    /// (`posts/a.png`, `archive.png`, `tags/rust.png`), so pages never share a
    /// file even when a post and a static page have the same name.
    file: String,
    /// `{{kind}}` in og.html: post, page, index, archive, tags or tag. Lets a
    /// card's CSS lay out list pages differently from posts.
    kind: &'static str,
    title: String,
    date: String,
    description: String,
}

/// Renders the OGP images for `pages` and returns the `file`s that were
/// written. Images are only made when they can be used: og:image needs an
/// absolute URL (so `base_url` must be known), the theme must have an
/// `og.html`, and neither site.conf (`og_image: off`) nor the caller (`serve`)
/// has turned them off. Problems become notes or warnings; the build never
/// fails because of an image.
fn og_images(
    config: &Config,
    site: &SiteMeta,
    templates: &Templates,
    pages: &[OgPage],
    r: &mut Renderer,
    written: &mut Vec<PathBuf>,
    (warnings, notes): (&mut Vec<String>, &mut Vec<String>),
) -> HashSet<String> {
    let mut done = HashSet::new();
    let Some(template) = &templates.og else {
        return done;
    };
    if !config.og_images || !site.og_image || site.base_url.is_none() || pages.is_empty() {
        return done;
    }
    let dir = config.out_dir.join("og");
    let cards: Vec<og::Card> = pages
        .iter()
        .map(|page| {
            let context = r
                .context("")
                .with("kind", page.kind)
                .with("title", escape_html(&page.title))
                .with("date", escape_html(&page.date))
                .with("description", escape_html(&page.description));
            og::Card {
                html: r.render(template, &context),
                target: dir.join(&page.file),
            }
        })
        .collect();

    let chrome = og::find_chrome();
    let outcome = og::render(&cards, chrome.as_deref());
    for (page, card) in pages.iter().zip(&cards) {
        if outcome.written.contains(&card.target) {
            done.insert(page.file.clone());
        }
    }
    let missing = cards.len() - outcome.written.len();
    if chrome.is_none() && missing > 0 {
        notes.push(format!(
            "Chrome not found: skipped {missing} OGP image(s) \
             (install Google Chrome or set BARE_CHROME to its path)"
        ));
    }
    if !outcome.written.is_empty() {
        notes.push(format!(
            "OGP images: {} written ({} from cache)",
            outcome.written.len(),
            outcome.cached
        ));
    }
    // Warnings never fail a build (only broken links do in `check`), so a
    // failed image is reported without stopping a deploy.
    warnings.extend(outcome.warnings);
    written.extend(outcome.written);
    done
}

/// The root-relative URL (`/og/posts/a.png`) of a generated image, or `None`
/// if `file` was not generated. Each path segment is percent-encoded, as page
/// URLs are, since slugs and tags may contain spaces or Japanese.
fn og_url(done: &HashSet<String>, file: &str) -> Option<String> {
    done.contains(file).then(|| {
        let encoded: Vec<String> = file.split('/').map(encode_path).collect();
        format!("/og/{}", encoded.join("/"))
    })
}

/// Copies the merged contents of `roots` (highest priority first) into `to`.
///
/// For each relative path the highest-priority file wins, so `content/static/`
/// overrides the theme and the theme overrides the default theme file by file.
/// Stylesheets are bundled (their imports resolved through the same roots) and
/// CSS partials are skipped. Returns the number of files written.
fn copy_static(
    roots: &[PathBuf],
    to: &Path,
    warnings: &mut Vec<String>,
) -> Result<usize, BuildError> {
    let mut files = BTreeMap::new();
    for root in roots {
        collect_files(root, Path::new(""), &mut files)?;
    }
    let mut count = 0;
    for (relative, source) in files {
        if css::is_partial(&source) {
            continue;
        }
        let target = to.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        if source.extension().is_some_and(|ext| ext == "css") {
            let bundled = css::bundle(&source, roots, warnings).at(&source)?;
            fs::write(&target, bundled).at(&target)?;
        } else {
            fs::copy(&source, &target).at(&source)?;
        }
        count += 1;
    }
    Ok(count)
}

/// Adds every file under `dir` to `files` as (path relative to the root, source),
/// keeping an entry already added by a higher-priority root.
fn collect_files(
    dir: &Path,
    relative: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), BuildError> {
    let current = dir.join(relative);
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&current).at(&current)? {
        let entry = entry.at(&current)?;
        let child = relative.join(entry.file_name());
        if entry.file_type().at(&entry.path())?.is_dir() {
            collect_files(dir, &child, files)?;
        } else {
            files.entry(child).or_insert_with(|| entry.path());
        }
    }
    Ok(())
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
            // Posts live in posts/, one level below the site root.
            let rendered = parser::render_markdown(&doc.body, "../");
            Post {
                title: meta.get("title").unwrap_or(&slug).to_string(),
                date,
                description: optional(meta, "description"),
                image: optional(meta, "image"),
                tags: meta.tags(),
                headings: rendered.headings,
                excerpt: text::excerpt(&rendered.html),
                reading_minutes: text::reading_minutes(&rendered.html),
                html: rendered.html,
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
        // Pages are written at the site root.
        let rendered = parser::render_markdown(&doc.body, "");
        pages.push(Page {
            title: meta.get("title").unwrap_or(&slug).to_string(),
            description: optional(meta, "description"),
            image: optional(meta, "image"),
            nav: meta.get("nav").and_then(|v| v.trim().parse().ok()),
            headings: rendered.headings,
            html: rendered.html,
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

// Arguments accepted by the placeholders that produce markup. Leaving them out
// gives the default output; see "トークンの引数" in content/posts/customize-design.md.
//
// The defaults in the render functions below must produce exactly the markup
// that was output before arguments existed, so forks that never use arguments
// see no change. Keep that post in sync when adding an argument.
const NAV_ARGS: Spec = &[("class", Kind::Text), ("label", Kind::Text)];
const TAGS_ARGS: Spec = &[
    ("class", Kind::Text),
    ("item_class", Kind::Text),
    ("label", Kind::Text),
    ("prefix", Kind::Text),
];
const TOC_ARGS: Spec = &[("class", Kind::Text)];
const POST_LIST_ARGS: Spec = &[
    ("item_class", Kind::Text),
    ("desc", Kind::Bool),
    ("tags", Kind::Bool),
];
const PAGINATION_ARGS: Spec = &[
    ("class", Kind::Text),
    ("current_class", Kind::Text),
    ("prev", Kind::Text),
    ("next", Kind::Text),
    ("prev_label", Kind::Text),
    ("next_label", Kind::Text),
    ("gap", Kind::Text),
];
const ARCHIVE_ARGS: Spec = &[
    ("class", Kind::Text),
    ("heading", Kind::Heading),
    ("count_prefix", Kind::Text),
    ("count_suffix", Kind::Text),
];
const TAG_LIST_ARGS: Spec = &[
    ("link_class", Kind::Text),
    ("count_prefix", Kind::Text),
    ("count_suffix", Kind::Text),
];

/// ` name="value"`, or nothing when `value` is empty.
///
/// `value` is inserted unescaped. Pass only template arguments and fixed
/// defaults here; anything from posts or site.conf must go through
/// `escape_html` first.
fn attr(name: &str, value: &str) -> String {
    match value {
        "" => String::new(),
        value => format!(" {name}=\"{value}\""),
    }
}

/// Text read by screen readers only (the `.visually-hidden` class in
/// `_utilities.css`). Like [`attr`], `text` is inserted unescaped.
fn hidden(text: &str) -> String {
    match text {
        "" => String::new(),
        text => format!("<span class=\"visually-hidden\">{text}</span>"),
    }
}

/// A post count, read as e.g. "2026、3件" next to the year or tag before it.
/// Without the hidden prefix, screen readers run the label and the number
/// together ("20263"), since they sit side by side with no text between.
fn render_count(count: usize, args: &Args) -> String {
    format!(
        "<span class=\"tag-count\">{}{count}{}</span>",
        hidden(args.text("count_prefix", "、")),
        hidden(args.text("count_suffix", "件"))
    )
}

/// Renders `{{nav}}`: links to the pages with `nav` in their frontmatter.
fn render_nav(nav: &[(String, String)], root: &str, current: Option<&str>, args: &Args) -> String {
    if nav.is_empty() {
        return String::new();
    }
    let links: String = nav
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
    format!(
        "<nav{}{}>{links}</nav>",
        attr("class", args.text("class", "site-nav")),
        attr("aria-label", args.text("label", "サイト"))
    )
}

/// Renders tags, as links to their tag pages when `root` is given.
fn render_tags(tags: &[String], root: Option<&str>, args: &Args) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let item_class = attr("class", args.text("item_class", "tag"));
    // Inside the link text, so a `#` prefix is part of what is clicked and read.
    let prefix = args.text("prefix", "");
    let items: String = tags
        .iter()
        .map(|tag| match root {
            Some(root) => format!(
                "<li{item_class}><a href=\"{root}tags/{}.html\">{prefix}{}</a></li>",
                escape_html(&encode_path(&tag_file(tag))),
                escape_html(tag)
            ),
            None => format!("<li{item_class}>{prefix}{}</li>", escape_html(tag)),
        })
        .collect();
    format!(
        "<ul{}{}>{items}</ul>",
        attr("class", args.text("class", "tags")),
        attr("aria-label", args.text("label", "タグ"))
    )
}

/// Renders `<li>` items for a post list on a page `root` away from the site root.
fn render_post_list(posts: &[&Post], root: &str, link_tags: bool, args: &Args) -> String {
    let item_class = attr("class", args.text("item_class", "post-item"));
    let show_desc = args.flag("desc", true);
    let show_tags = args.flag("tags", true);
    let mut out = String::new();
    for post in posts {
        let description = match post.summary() {
            summary if show_desc && !summary.is_empty() => {
                format!("<p class=\"post-desc\">{}</p>", escape_html(summary))
            }
            _ => String::new(),
        };
        let tags = match show_tags {
            // Tags inside a list use the default markup; `{{post_list}}`
            // arguments do not reach them.
            true => render_tags(&post.tags, link_tags.then_some(root), &Args::default()),
            false => String::new(),
        };
        out.push_str(&format!(
            "<li{item_class}>\
             <time datetime=\"{date}\">{date}</time>\
             <a href=\"{root}{href}\">{title}</a>\
             {description}{tags}</li>\n",
            date = escape_html(&post.date),
            href = escape_html(&post.path()),
            title = escape_html(&post.title),
        ));
    }
    out
}

/// Renders `{{pagination}}`: previous/next links and page numbers, with
/// ellipses for long ranges. Empty when everything fits on one page.
fn render_pagination(current: usize, count: usize, root: &str, args: &Args) -> String {
    if count <= 1 {
        return String::new();
    }
    // Page 1 is linked as its directory (`../` or `./`), not `index.html`, so
    // the URL matches the one the site is entered by.
    let href = |n: usize| match (n, root) {
        (1, "") => "./".to_string(),
        (1, root) => root.to_string(),
        (n, root) => format!("{root}page/{n}.html"),
    };
    let mut items = Vec::new();
    // The default link text is just an arrow, which screen readers announce as
    // "left arrow"; the aria-label gives the link its real name.
    if current > 1 {
        items.push(format!(
            "<li><a rel=\"prev\"{} href=\"{}\">{}</a></li>",
            attr("aria-label", args.text("prev_label", "前のページ")),
            href(current - 1),
            args.text("prev", "←")
        ));
    }
    let gap = format!("<li class=\"gap\">{}</li>", args.text("gap", "…"));
    let current_class = attr("class", args.text("current_class", ""));
    let mut last_shown = 0;
    for n in 1..=count {
        // First, last, and the pages next to the current one.
        if n != 1 && n != count && n.abs_diff(current) > 1 {
            continue;
        }
        if n > last_shown + 1 {
            items.push(gap.clone());
        }
        items.push(if n == current {
            format!("<li><span{current_class} aria-current=\"page\">{n}</span></li>")
        } else {
            format!("<li><a href=\"{}\">{n}</a></li>", href(n))
        });
        last_shown = n;
    }
    if current < count {
        items.push(format!(
            "<li><a rel=\"next\"{} href=\"{}\">{}</a></li>",
            attr("aria-label", args.text("next_label", "次のページ")),
            href(current + 1),
            args.text("next", "→")
        ));
    }
    format!(
        "<ol{}>{}</ol>",
        attr("class", args.text("class", "pagination")),
        items.join("")
    )
}

/// Renders posts grouped by year, newest first, for `{{archive}}`.
fn render_archive(posts: &[Post], link_tags: bool, args: &Args) -> String {
    let class = attr("class", args.text("class", "archive-year"));
    let heading = args.text("heading", "h2");
    let mut out = String::new();
    let mut start = 0;
    while start < posts.len() {
        let year = posts[start].year();
        let end = posts[start..]
            .iter()
            .position(|post| post.year() != year)
            .map_or(posts.len(), |n| start + n);
        let group: Vec<&Post> = posts[start..end].iter().collect();
        let label = escape_html(if year.is_empty() { "----" } else { year });
        out.push_str(&format!(
            "<section{class}>\n<{heading} id=\"y{label}\">{label}{count}</{heading}>\n\
             <ul class=\"post-list\">\n{list}</ul>\n</section>\n",
            count = render_count(group.len(), args),
            // The lists inside the archive use the default markup; `{{archive}}`
            // arguments only cover the year sections.
            list = render_post_list(&group, "", link_tags, &Args::default()),
        ));
        start = end;
    }
    out
}

/// Renders `<li>` items for the tag index, most used tags first.
fn render_tag_list(groups: &[TagGroup], root: &str, args: &Args) -> String {
    let link_class = attr("class", args.text("link_class", "tag"));
    let mut sorted: Vec<&TagGroup> = groups.iter().collect();
    sorted.sort_by_key(|group| std::cmp::Reverse(group.posts.len()));
    let mut out = String::new();
    for group in sorted {
        out.push_str(&format!(
            "<li><a{link_class} href=\"{root}{}\">{}</a>{}</li>\n",
            escape_html(&group.path()),
            escape_html(&group.name),
            render_count(group.posts.len(), args)
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
            per_page: None,
            archive_title: "Archive".into(),
            tags_title: "Tags".into(),
            not_found_title: "404 Not Found".into(),
            og_image: true,
            theme: None,
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

    #[test]
    fn renders_pagination() {
        assert_eq!(render_pagination(1, 1, "", &Args::default()), "");
        assert_eq!(
            render_pagination(1, 2, "", &Args::default()),
            "<ol class=\"pagination\"><li><span aria-current=\"page\">1</span></li>\
             <li><a href=\"page/2.html\">2</a></li>\
             <li><a rel=\"next\" aria-label=\"次のページ\" href=\"page/2.html\">→</a></li></ol>"
        );
        let middle = render_pagination(5, 9, "../", &Args::default());
        assert!(middle.starts_with("<ol class=\"pagination\"><li><a rel=\"prev\" aria-label=\"前のページ\" href=\"../page/4.html\">←</a></li>\
             <li><a href=\"../\">1</a></li><li class=\"gap\">…</li>\
             <li><a href=\"../page/4.html\">4</a></li><li><span aria-current=\"page\">5</span></li>"));
        assert!(middle.ends_with(
            "<li class=\"gap\">…</li><li><a href=\"../page/9.html\">9</a></li>\
             <li><a rel=\"next\" aria-label=\"次のページ\" href=\"../page/6.html\">→</a></li></ol>"
        ));
    }

    /// Creates `files` (relative path, contents) under a fresh temporary directory.
    fn temp_tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = env::temp_dir().join(format!("bare-site-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (path, contents) in files {
            let path = dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        dir
    }

    #[test]
    fn resolves_theme_layers() {
        let dir = temp_tree(
            "layers",
            &[
                ("themes/default/templates/post.html", ""),
                ("themes/mine/templates/post.html", ""),
            ],
        );
        let mut warnings = Vec::new();

        let layers = Layers::resolve(&dir, Some("mine"), &mut warnings).unwrap();
        assert_eq!(
            layers.templates,
            [
                dir.join("themes/mine/templates"),
                dir.join("themes/default/templates")
            ]
        );
        assert_eq!(
            layers.statics,
            [
                dir.join("static"),
                dir.join("themes/mine/static"),
                dir.join("themes/default/static")
            ]
        );

        // No theme set: the default theme alone, not listed twice.
        let layers = Layers::resolve(&dir, None, &mut warnings).unwrap();
        assert_eq!(layers.templates, [dir.join("themes/default/templates")]);
        assert!(warnings.is_empty());

        // A leftover content/templates/ still wins, with a warning.
        fs::create_dir_all(dir.join("templates")).unwrap();
        let layers = Layers::resolve(&dir, Some("mine"), &mut warnings).unwrap();
        assert_eq!(layers.templates[0], dir.join("templates"));
        assert_eq!(warnings.len(), 1);

        assert!(Layers::resolve(&dir, Some("missing"), &mut warnings).is_err());
        assert!(Layers::resolve(&dir, Some("../mine"), &mut warnings).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_the_old_layout_without_themes() {
        let dir = temp_tree("legacy", &[("templates/post.html", "")]);
        let mut warnings = Vec::new();
        let layers = Layers::resolve(&dir, None, &mut warnings).unwrap();
        assert_eq!(layers.templates, [dir.join("templates")]);
        assert_eq!(layers.statics, [dir.join("static")]);
        assert!(warnings.is_empty());
        // Asking for a theme without themes/ is a mistake, not a silent fallback.
        assert!(Layers::resolve(&dir, Some("mine"), &mut warnings).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn merges_static_layers_file_by_file() {
        let dir = temp_tree(
            "static",
            &[
                ("site/favicon.svg", "site"),
                ("site/css/custom.css", "custom {}"),
                ("theme/css/_tokens.css", "theme-tokens {}"),
                ("theme/favicon.svg", "theme"),
                (
                    "default/css/style.css",
                    "@import \"_tokens.css\";\n@import \"_base.css\";",
                ),
                ("default/css/_tokens.css", "default-tokens {}"),
                ("default/css/_base.css", "default-base {}"),
            ],
        );
        let roots = [dir.join("site"), dir.join("theme"), dir.join("default")];
        let out = dir.join("out");
        let mut warnings = Vec::new();
        let count = copy_static(&roots, &out, &mut warnings).unwrap();

        assert_eq!(
            count, 3,
            "favicon, custom.css and style.css; partials are skipped"
        );
        assert_eq!(fs::read_to_string(out.join("favicon.svg")).unwrap(), "site");
        assert_eq!(
            fs::read_to_string(out.join("css/style.css")).unwrap(),
            "theme-tokens {}\ndefault-base {}\n"
        );
        assert!(!out.join("css/_tokens.css").exists());
        assert!(warnings.is_empty(), "{warnings:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Builds a whole site through `build`: theme fallback, token arguments,
    /// CSS overrides, site.conf page names and warnings, end to end.
    #[test]
    fn builds_a_themed_site_end_to_end() {
        let dir = temp_tree(
            "build",
            &[
                (
                    "content/site.conf",
                    "title: T\ntheme: mine\nper_page: 1\narchive_title: 年別\n",
                ),
                (
                    "content/posts/a.md",
                    "---\ntitle: A\ndate: 2026-01-02\ntags: [x]\n---\n\n## One\n\ntext\n\n## Two\n",
                ),
                ("content/posts/b.md", "---\ntitle: B\ndate: 2025-01-01\n---\n\nb\n"),
                ("content/static/css/custom.css", "custom {}"),
                (
                    "content/themes/default/templates/layout.html",
                    "<title>{{page_title}}</title><a href=\"{{root}}\">home</a>{{content}}",
                ),
                (
                    "content/themes/default/templates/post.html",
                    "<h1>{{title}}</h1>{{tags}}{{toc}}{{content}}",
                ),
                ("content/themes/default/templates/index.html", "{{post_list}}{{pagination}}"),
                (
                    "content/themes/default/templates/archive.html",
                    "{{archive heading=\"h3\" count_suffix=\" posts\"}}",
                ),
                ("content/themes/default/templates/tag.html", "{{post_list}}"),
                ("content/themes/default/static/css/style.css", "@import \"_tokens.css\";"),
                ("content/themes/default/static/css/_tokens.css", "default {}"),
                // The theme replaces two templates and one CSS partial only.
                (
                    "content/themes/mine/templates/post.html",
                    "<h1 class=\"mine\">{{title}}</h1>{{tags class=\"chips\" prefix=\"#\"}}{{toc class=\"\"}}{{content}}",
                ),
                (
                    "content/themes/mine/templates/index.html",
                    "{{post_list item_class=\"card\" desc=\"false\"}}{{pagination next=\"Older\" bogus=\"1\"}}",
                ),
                ("content/themes/mine/static/css/_tokens.css", "mine {}"),
            ],
        );
        let out = dir.join("dist");
        let report = build(&Config {
            content_dir: dir.join("content"),
            out_dir: out.clone(),
            clean: false,
            drafts: false,
            base_url: None,
            og_images: true,
        })
        .unwrap();
        let read = |path: &str| fs::read_to_string(out.join(path)).unwrap();

        let post = read("posts/a.html");
        assert!(post.contains("<h1 class=\"mine\">A</h1>"));
        assert!(post.contains(
            "<ul class=\"chips\" aria-label=\"タグ\"><li class=\"tag\"><a href=\"../tags/x.html\">#x</a></li></ul>"
        ));
        assert!(post.contains("<ol>\n<li><a href=\"#one\">One</a>"));

        let index = read("index.html");
        assert!(index.contains("<li class=\"card\"><time datetime=\"2026-01-02\">"));
        assert!(index
            .contains("<a rel=\"next\" aria-label=\"次のページ\" href=\"page/2.html\">Older</a>"));
        assert!(out.join("page/2.html").is_file());
        // Page 2 links back to page 1 as a directory, not index.html.
        assert!(read("page/2.html").contains("<li><a href=\"../\">1</a></li>"));
        // Root-level pages get `./` for {{root}}, so the home link is never empty.
        assert!(read("archive.html").contains("<a href=\"./\">home</a>"));
        assert!(read("posts/a.html").contains("<a href=\"../\">home</a>"));

        let archive = read("archive.html");
        assert!(archive.contains("<title>年別 · T</title>"));
        assert!(archive.contains(
            "<h3 id=\"y2026\">2026<span class=\"tag-count\"><span class=\"visually-hidden\">、</span>1\
             <span class=\"visually-hidden\"> posts</span></span></h3>"
        ));

        // The default style.css picks up the theme's partial; partials are not copied.
        assert_eq!(read("css/style.css"), "mine {}\n");
        assert_eq!(read("css/custom.css"), "custom {}\n");
        assert!(!out.join("css/_tokens.css").exists());

        // Warnings name the theme file that actually supplied the template.
        let bogus: Vec<&String> = report
            .warnings
            .iter()
            .filter(|w| w.contains("unknown argument bogus"))
            .collect();
        assert_eq!(bogus.len(), 1, "{:?}", report.warnings);
        assert!(bogus[0].contains(&format!("themes{}mine", std::path::MAIN_SEPARATOR)));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn renders_markup_with_arguments() {
        let nav = vec![("about.html".to_string(), "About".to_string())];
        assert_eq!(
            render_nav(&nav, "../", Some("about.html"), &Args::default()),
            "<nav class=\"site-nav\" aria-label=\"サイト\">\
             <a href=\"../about.html\" aria-current=\"page\">About</a></nav>"
        );
        let args = Args::parse("class=\"\" label=\"Main\"").unwrap();
        assert_eq!(
            render_nav(&nav, "", None, &args),
            "<nav aria-label=\"Main\"><a href=\"about.html\">About</a></nav>"
        );

        let groups = [TagGroup {
            name: "rust".into(),
            file: "rust".into(),
            posts: Vec::new(),
        }];
        let args =
            Args::parse("link_class=\"chip\" count_prefix=\"\" count_suffix=\" posts\"").unwrap();
        assert_eq!(
            render_tag_list(&groups, "../", &args),
            "<li><a class=\"chip\" href=\"../tags/rust.html\">rust</a><span class=\"tag-count\">0\
             <span class=\"visually-hidden\"> posts</span></span></li>\n"
        );

        let args =
            Args::parse("class=\"pg\" current_class=\"now\" prev=\"Newer\" gap=\"...\"").unwrap();
        let html = render_pagination(5, 9, "", &args);
        assert!(html.starts_with("<ol class=\"pg\"><li><a rel=\"prev\" aria-label=\"前のページ\" href=\"page/4.html\">Newer</a>"));
        assert!(html.contains("<li class=\"gap\">...</li>"));
        assert!(html.contains("<span class=\"now\" aria-current=\"page\">5</span>"));
    }

    fn post(slug: &str, image: Option<&str>) -> Post {
        Post {
            slug: slug.into(),
            title: slug.to_uppercase(),
            date: "2026-01-02".into(),
            description: None,
            image: image.map(str::to_string),
            tags: vec!["x".into()],
            html: String::new(),
            headings: Vec::new(),
            excerpt: String::new(),
            reading_minutes: 1,
        }
    }

    #[test]
    fn chooses_pages_for_ogp_images() {
        let templates = Templates {
            layout: Template::new(""),
            post: Template::new(""),
            index: Template::new(""),
            page: None,
            archive: Some(Template::new("")),
            tag: Some(Template::new("")),
            tags: Some(Template::new("")),
            not_found: None,
            og: Some(Template::new("")),
        };
        let posts = [post("a", None), post("b", Some("/images/b.png"))];
        let pages = [Page {
            slug: "about".into(),
            title: "About".into(),
            description: Some("Me".into()),
            image: None,
            nav: None,
            html: String::new(),
            headings: Vec::new(),
        }];
        let groups = group_by_tag(&posts);
        let files = |site: &SiteMeta| -> Vec<(String, &'static str)> {
            og_requests(site, &templates, &posts, &pages, &groups)
                .into_iter()
                .map(|page| (page.file, page.kind))
                .collect()
        };

        // Post b has its own image; every other page gets a card.
        let mut site = site(None);
        assert_eq!(
            files(&site),
            [
                ("posts/a.png".to_string(), "post"),
                ("about.png".to_string(), "page"),
                ("index.png".to_string(), "index"),
                ("archive.png".to_string(), "archive"),
                ("tags/x.png".to_string(), "tag"),
                ("tags/index.png".to_string(), "tags"),
            ]
        );

        // A site-wide image stays the image of the list pages.
        site.image = Some("/images/site.png".into());
        assert_eq!(
            files(&site),
            [
                ("posts/a.png".to_string(), "post"),
                ("about.png".to_string(), "page")
            ]
        );
    }

    #[test]
    fn encodes_ogp_image_urls() {
        let done: HashSet<String> = ["posts/日本語 a.png".to_string()].into();
        assert_eq!(
            og_url(&done, "posts/日本語 a.png").unwrap(),
            "/og/posts/%E6%97%A5%E6%9C%AC%E8%AA%9E%20a.png"
        );
        assert_eq!(og_url(&done, "posts/other.png"), None);
    }
}
