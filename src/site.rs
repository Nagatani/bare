//! Site orchestration: discover posts, render them through templates, write `dist/`.

use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::feed;
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

struct Templates {
    layout: Template,
    post: Template,
    index: Template,
    /// Optional, so forks with older template sets keep building.
    not_found: Option<Template>,
}

impl Templates {
    fn load(dir: &Path) -> Result<Self, BuildError> {
        let load = |name: &str| {
            let path = dir.join(name);
            Template::load(&path).at(&path)
        };
        let not_found_path = dir.join("404.html");
        Ok(Self {
            layout: load("layout.html")?,
            post: load("post.html")?,
            index: load("index.html")?,
            not_found: match not_found_path.exists() {
                true => Some(load("404.html")?),
                false => None,
            },
        })
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
    let (mut posts, drafts_skipped) = load_posts(
        &config.content_dir.join("posts"),
        config.drafts,
        &mut warnings,
    )?;
    // Newest first; ISO dates (YYYY-MM-DD) sort correctly as strings.
    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));

    if config.clean && config.out_dir.exists() {
        ensure_safe_to_clean(config)?;
        fs::remove_dir_all(&config.out_dir).at(&config.out_dir)?;
    }

    // Without `clean`, only the posts directory is cleared, so stale pages from
    // deleted posts disappear without touching anything else in the output dir.
    let posts_out = config.out_dir.join("posts");
    if posts_out.exists() {
        fs::remove_dir_all(&posts_out).at(&posts_out)?;
    }
    fs::create_dir_all(&posts_out).at(&posts_out)?;

    // Static files go in first, so a generated page wins over a same-named file.
    let static_files = copy_dir(&config.content_dir.join("static"), &config.out_dir)?;

    let base = site.context();
    let mut written = Vec::new();
    let mut write = |path: PathBuf, contents: String| -> Result<(), BuildError> {
        fs::write(&path, contents).at(&path)?;
        written.push(path);
        Ok(())
    };

    for post in &posts {
        let root = "../";
        let body = templates.post.render(
            &base
                .clone()
                .with("title", escape_html(&post.title))
                .with("date", escape_html(&post.date))
                .with(
                    "description",
                    escape_html(post.description.as_deref().unwrap_or("")),
                )
                .with("tags", render_tags(&post.tags))
                .with("content", post.html.as_str()),
        );
        let meta = site.meta_tags(&PageMeta {
            title: &post.title,
            description: post.description.as_deref(),
            path: Some(&post.path()),
            image: post.image.as_deref(),
            published: Some(&post.date),
            root,
        });
        let page = templates.layout.render(
            &base
                .clone()
                .with("title", escape_html(&post.title))
                .with(
                    "page_title",
                    escape_html(&format!("{} · {}", post.title, site.title)),
                )
                .with("meta", meta)
                .with("root", root)
                .with("content", body),
        );
        write(posts_out.join(format!("{}.html", post.slug)), page)?;
    }

    let body = templates.index.render(
        &base
            .clone()
            .with("post_list", render_post_list(&posts))
            .with("post_count", posts.len().to_string()),
    );
    let meta = site.meta_tags(&PageMeta {
        title: &site.title,
        description: None,
        path: Some(""),
        image: None,
        published: None,
        root: "",
    });
    let page = templates.layout.render(
        &base
            .clone()
            .with("title", escape_html(&site.title))
            .with("page_title", escape_html(&site.title))
            .with("meta", meta)
            .with("root", "")
            .with("content", body),
    );
    write(config.out_dir.join("index.html"), page)?;

    if let Some(not_found) = &templates.not_found {
        // GitHub Pages serves 404.html for a missing URL at any depth, so links
        // must be root-relative (e.g. `/bare/`) rather than `../`.
        let root = site.base_path();
        let body = not_found.render(&base.clone().with("root", root.as_str()));
        let meta = site.meta_tags(&PageMeta {
            title: "404 Not Found",
            description: None,
            path: None,
            image: None,
            published: None,
            root: &root,
        });
        let page = templates.layout.render(
            &base
                .clone()
                .with("title", "404 Not Found")
                .with(
                    "page_title",
                    escape_html(&format!("404 Not Found · {}", site.title)),
                )
                .with("meta", meta)
                .with("root", root.as_str())
                .with("content", body),
        );
        write(config.out_dir.join("404.html"), page)?;
    }

    if site.base_url.is_some() {
        write(config.out_dir.join("feed.xml"), feed::atom(&site, &posts))?;
        write(
            config.out_dir.join("sitemap.xml"),
            feed::sitemap(&site, &posts),
        )?;
    } else {
        notes.push(
            "base_url is not set: skipped feed.xml, sitemap.xml and absolute URLs \
             (pass --base-url or set base_url in site.conf; CI sets it automatically)"
                .to_string(),
        );
    }

    Ok(BuildReport {
        posts: posts.len(),
        drafts_skipped,
        written,
        static_files,
        warnings,
        notes,
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

/// Reads and renders every `*.md` file directly inside `dir`, returning the posts
/// and the number of drafts left out.
/// A missing directory means no posts yet (git does not keep empty directories).
fn load_posts(
    dir: &Path,
    include_drafts: bool,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Post>, usize), BuildError> {
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

    let mut posts = Vec::with_capacity(paths.len());
    let mut drafts = 0;
    for path in paths {
        let source = fs::read_to_string(&path).at(&path)?;
        let doc = parser::parse_document(&source);
        let meta = &doc.frontmatter;

        let is_draft = meta
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
        let date = meta.get("date").unwrap_or_default().to_string();
        if !is_iso_date(&date) {
            warnings.push(format!(
                "{}: date {date:?} is not YYYY-MM-DD; post order and feed dates may be wrong",
                path.display()
            ));
        }
        let optional = |key: &str| meta.get(key).filter(|v| !v.is_empty()).map(str::to_string);

        posts.push(Post {
            title: meta.get("title").unwrap_or(&slug).to_string(),
            date,
            description: optional("description"),
            image: optional("image"),
            tags: meta.tags(),
            // Posts live in posts/, one level below the site root.
            html: parser::markdown_to_html(&doc.body, "../"),
            slug,
        });
    }
    Ok((posts, drafts))
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

fn render_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let items: Vec<String> = tags
        .iter()
        .map(|tag| format!("<li class=\"tag\">{}</li>", escape_html(tag)))
        .collect();
    format!("<ul class=\"tags\">{}</ul>", items.join(""))
}

fn render_post_list(posts: &[Post]) -> String {
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
             <a href=\"{href}\">{title}</a>\
             {description}{tags}</li>\n",
            date = escape_html(&post.date),
            href = escape_html(&post.path()),
            title = escape_html(&post.title),
            tags = render_tags(&post.tags),
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
