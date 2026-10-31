//! Site orchestration: discover posts, render them through templates, write `dist/`.

use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::parser::{self, escape_html};
use crate::template::{Context, Template};

/// Where to read sources from and where to write the generated site.
#[derive(Debug, Clone)]
pub struct Config {
    pub content_dir: PathBuf,
    pub out_dir: PathBuf,
    /// Delete the whole output directory before building.
    pub clean: bool,
}

/// A parsed and rendered post, ready to be placed into templates.
#[derive(Debug)]
struct Post {
    slug: String,
    title: String,
    date: String,
    tags: Vec<String>,
    html: String,
}

/// Summary of a finished build.
#[derive(Debug)]
pub struct BuildReport {
    pub posts: usize,
    pub written: Vec<PathBuf>,
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
}

impl Templates {
    fn load(dir: &Path) -> Result<Self, BuildError> {
        let load = |name: &str| {
            let path = dir.join(name);
            Template::load(&path).at(&path)
        };
        Ok(Self {
            layout: load("layout.html")?,
            post: load("post.html")?,
            index: load("index.html")?,
        })
    }
}

/// Builds the whole site described by `config`.
pub fn build(config: &Config) -> Result<BuildReport, BuildError> {
    let templates = Templates::load(&config.content_dir.join("templates"))?;
    let site = load_site_context(&config.content_dir.join("site.conf"))?;
    let mut posts = load_posts(&config.content_dir.join("posts"))?;
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

    let mut written = Vec::with_capacity(posts.len() + 1);

    for post in &posts {
        let body = templates.post.render(
            &site
                .clone()
                .with("title", escape_html(&post.title))
                .with("date", escape_html(&post.date))
                .with("tags", render_tags(&post.tags))
                .with("content", post.html.as_str()),
        );
        let page = templates.layout.render(
            &site
                .clone()
                .with("title", escape_html(&post.title))
                .with("root", "../")
                .with("content", body),
        );
        let path = posts_out.join(format!("{}.html", post.slug));
        fs::write(&path, page).at(&path)?;
        written.push(path);
    }

    let body = templates.index.render(
        &site
            .clone()
            .with("post_list", render_post_list(&posts))
            .with("post_count", posts.len().to_string()),
    );
    let page = templates.layout.render(
        &site
            .clone()
            .with("title", "Home")
            .with("root", "")
            .with("content", body),
    );
    let index_path = config.out_dir.join("index.html");
    fs::write(&index_path, page).at(&index_path)?;
    written.push(index_path);

    Ok(BuildReport {
        posts: posts.len(),
        written,
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

/// Reads `site.conf` into the `{{site_*}}` values shared by every template.
/// The file is optional; missing keys fall back to defaults.
fn load_site_context(path: &Path) -> Result<Context, BuildError> {
    let conf = match fs::read_to_string(path) {
        Ok(text) => parser::parse_key_values(&text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Default::default(),
        Err(err) => return Err(err).at(path),
    };
    let value = |key: &str, default: &str| escape_html(conf.get(key).unwrap_or(default));
    Ok(Context::new()
        .with("site_title", value("title", "bare"))
        .with("site_description", value("description", ""))
        .with("site_author", value("author", ""))
        .with("site_lang", value("lang", "ja")))
}

/// Reads and renders every `*.md` file directly inside `dir`.
/// A missing directory means no posts yet (git does not keep empty directories).
fn load_posts(dir: &Path) -> Result<Vec<Post>, BuildError> {
    if !dir.exists() {
        return Ok(Vec::new());
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
    for path in paths {
        let source = fs::read_to_string(&path).at(&path)?;
        let doc = parser::parse_document(&source);
        let slug = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let meta = &doc.frontmatter;
        posts.push(Post {
            title: meta.get("title").unwrap_or(&slug).to_string(),
            date: meta.get("date").unwrap_or_default().to_string(),
            tags: meta.tags(),
            html: parser::markdown_to_html(&doc.body),
            slug,
        });
    }
    Ok(posts)
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
        out.push_str(&format!(
            "<li class=\"post-item\">\
             <time datetime=\"{date}\">{date}</time>\
             <a href=\"posts/{slug}.html\">{title}</a>\
             {tags}</li>\n",
            date = escape_html(&post.date),
            slug = escape_html(&post.slug),
            title = escape_html(&post.title),
            tags = render_tags(&post.tags),
        ));
    }
    out
}
