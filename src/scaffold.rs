//! `bare new`: creates a Markdown file with frontmatter filled in.

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
}

/// Writes the new file under `content_dir` and returns its path. Posts start as
/// drafts, so they show up in `bare serve` but are not published until
/// `draft: true` is removed.
pub fn create(content_dir: &Path, options: &NewOptions) -> io::Result<PathBuf> {
    let slug = options.slug.trim().trim_end_matches(".md");
    if slug.is_empty() || slug.starts_with('.') || slug.contains(['/', '\\']) {
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
        };
        assert!(create(&dir, &bad).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
