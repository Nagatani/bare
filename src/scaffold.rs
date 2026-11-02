//! `bare new`: creates a Markdown file with frontmatter filled in, or a new
//! theme copied from the default one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// What `bare new` should create.
#[derive(Debug, PartialEq)]
pub struct NewOptions {
    /// File name without `.md`; also the URL of the page.
    pub slug: String,
    pub title: Option<String>,
    /// Create a standalone page in `pages/` instead of a post.
    pub page: bool,
    /// Create a theme in `themes/` (a copy of the default theme) instead of a post.
    pub theme: bool,
}

/// Writes the new file under `content_dir` and returns its path. Posts start as
/// drafts, so they show up in `bare serve` but are not published until
/// `draft: true` is removed.
pub fn create(content_dir: &Path, options: &NewOptions) -> io::Result<PathBuf> {
    if options.theme {
        return create_theme(content_dir, options.slug.trim());
    }
    let slug = options.slug.trim().trim_end_matches(".md");
    if !is_plain_name(slug) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid name `{}`: use something like `my-first-post`",
                options.slug
            ),
        ));
    }
    let dir = content_dir.join(if options.page { "pages" } else { "posts" });
    let path = dir.join(format!("{slug}.md"));
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }

    let title = options.title.as_deref().unwrap_or(slug).replace('"', "'");
    let text = if options.page {
        format!("---\ntitle: \"{title}\"\ndescription: \nnav: \n---\n\n")
    } else {
        format!(
            "---\ntitle: \"{title}\"\ndate: {}\ndescription: \ntags: \ndraft: true\n---\n\n",
            today()
        )
    };
    fs::create_dir_all(&dir)?;
    fs::write(&path, text)?;
    Ok(path)
}

/// Copies `themes/default/` to `themes/<name>/` and returns the new directory.
///
/// The copy is complete so every file is at hand to edit. Files left unchanged
/// can be deleted afterwards: the build falls back to the default theme for
/// them, and only then do upstream fixes to those files reach the site.
fn create_theme(content_dir: &Path, name: &str) -> io::Result<PathBuf> {
    if !is_plain_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid theme name `{name}`: use something like `my-theme`"),
        ));
    }
    let themes = content_dir.join("themes");
    let source = themes.join("default");
    if name == "default" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "`default` is the built-in theme; choose another name",
        ));
    }
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} not found: there is no default theme to copy",
                source.display()
            ),
        ));
    }
    let target = themes.join(name);
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", target.display()),
        ));
    }
    copy_tree(&source, &target)?;
    Ok(target)
}

/// Recursively copies the directory `from` to `to`, which must not exist yet.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// True for a name usable as a single file or directory name: not empty, not
/// hidden, and without path separators, so it cannot point outside its folder.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

/// Today's local date as `YYYY-MM-DD`. Uses the `date` command where available
/// (it knows the local time zone); otherwise falls back to UTC.
fn today() -> String {
    let local = Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|date| date.trim().to_string())
        .filter(|date| crate::site::is_iso_date(date));
    local.unwrap_or_else(|| {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        civil_date((secs / 86_400) as i64)
    })
}

/// Converts days since 1970-01-01 to `YYYY-MM-DD` (Howard Hinnant's algorithm).
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(19_783), "2024-03-01");
        assert_eq!(civil_date(20_758), "2026-11-01");
        assert!(crate::site::is_iso_date(&today()));
    }

    #[test]
    fn creates_posts_and_refuses_overwrite() {
        let dir = std::env::temp_dir().join(format!("bare-new-{}", std::process::id()));
        let options = NewOptions {
            slug: "hello".into(),
            title: Some("Say \"hi\"".into()),
            page: false,
            theme: false,
        };
        let path = create(&dir, &options).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("---\ntitle: \"Say 'hi'\"\ndate: "));
        assert!(text.contains("draft: true"));
        assert!(create(&dir, &options).is_err());

        let bad = NewOptions {
            slug: "../x".into(),
            title: None,
            page: true,
            theme: false,
        };
        assert!(create(&dir, &bad).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copies_the_default_theme() {
        let dir = std::env::temp_dir().join(format!("bare-new-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let theme = |name: &str| NewOptions {
            slug: name.into(),
            title: None,
            page: false,
            theme: true,
        };
        // Without a default theme there is nothing to copy.
        assert!(create(&dir, &theme("mine")).is_err());

        fs::create_dir_all(dir.join("themes/default/static/css")).unwrap();
        fs::write(dir.join("themes/default/static/css/_tokens.css"), "t {}").unwrap();
        let path = create(&dir, &theme("mine")).unwrap();
        assert_eq!(path, dir.join("themes/mine"));
        assert_eq!(
            fs::read_to_string(path.join("static/css/_tokens.css")).unwrap(),
            "t {}"
        );

        assert!(create(&dir, &theme("mine")).is_err(), "never overwrites");
        assert!(create(&dir, &theme("default")).is_err());
        assert!(create(&dir, &theme("../x")).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
