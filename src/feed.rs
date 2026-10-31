//! Atom feed and XML sitemap generation. Both need absolute URLs, so they are
//! only produced when the site's `base_url` is known.

use crate::parser::escape_html;
use crate::site::{is_iso_date, Post, SiteMeta};

/// Number of most recent posts included in the feed.
const FEED_ENTRIES: usize = 20;

/// Renders `feed.xml`. `posts` must be sorted newest first.
pub(crate) fn atom(site: &SiteMeta, posts: &[Post]) -> String {
    let home = site.url("").unwrap_or_default();
    let author = if site.author.is_empty() {
        &site.title
    } else {
        &site.author
    };
    let updated = posts
        .iter()
        .find(|post| is_iso_date(&post.date))
        .map(|post| timestamp(&post.date))
        .unwrap_or_else(|| timestamp(""));

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str(&format!(
        "<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:lang=\"{}\">\n",
        escape_html(&site.lang)
    ));
    out.push_str(&format!("  <title>{}</title>\n", escape_html(&site.title)));
    if !site.description.is_empty() {
        out.push_str(&format!(
            "  <subtitle>{}</subtitle>\n",
            escape_html(&site.description)
        ));
    }
    out.push_str(&format!(
        "  <link href=\"{}\" rel=\"alternate\"/>\n",
        escape_html(&home)
    ));
    out.push_str(&format!(
        "  <link href=\"{}feed.xml\" rel=\"self\"/>\n",
        escape_html(&home)
    ));
    out.push_str(&format!("  <id>{}</id>\n", escape_html(&home)));
    out.push_str(&format!("  <updated>{updated}</updated>\n"));
    out.push_str(&format!(
        "  <author><name>{}</name></author>\n",
        escape_html(author)
    ));
    out.push_str("  <generator uri=\"https://github.com/Nagatani/bare\">bare</generator>\n");

    for post in posts.iter().take(FEED_ENTRIES) {
        let url = site.url(&post.path()).unwrap_or_default();
        let date = timestamp(&post.date);
        out.push_str("  <entry>\n");
        out.push_str(&format!(
            "    <title>{}</title>\n",
            escape_html(&post.title)
        ));
        out.push_str(&format!("    <link href=\"{}\"/>\n", escape_html(&url)));
        out.push_str(&format!("    <id>{}</id>\n", escape_html(&url)));
        out.push_str(&format!("    <published>{date}</published>\n"));
        out.push_str(&format!("    <updated>{date}</updated>\n"));
        if let Some(description) = &post.description {
            out.push_str(&format!(
                "    <summary>{}</summary>\n",
                escape_html(description)
            ));
        }
        for tag in &post.tags {
            out.push_str(&format!("    <category term=\"{}\"/>\n", escape_html(tag)));
        }
        // Post HTML uses page-relative URLs (`../images/...`); xml:base lets
        // feed readers resolve them against the post's own location.
        out.push_str(&format!(
            "    <content type=\"html\" xml:base=\"{}\">{}</content>\n",
            escape_html(&site.url("posts/").unwrap_or_default()),
            escape_html(&post.html)
        ));
        out.push_str("  </entry>\n");
    }
    out.push_str("</feed>\n");
    out
}

/// Renders `sitemap.xml` listing the home page and every post.
pub(crate) fn sitemap(site: &SiteMeta, posts: &[Post]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    let newest = posts
        .iter()
        .map(|post| post.date.as_str())
        .find(|d| is_iso_date(d));
    push_url(&mut out, &site.url("").unwrap_or_default(), newest);
    for post in posts {
        let lastmod = Some(post.date.as_str()).filter(|d| is_iso_date(d));
        push_url(
            &mut out,
            &site.url(&post.path()).unwrap_or_default(),
            lastmod,
        );
    }

    out.push_str("</urlset>\n");
    out
}

fn push_url(out: &mut String, loc: &str, lastmod: Option<&str>) {
    out.push_str(&format!("  <url><loc>{}</loc>", escape_html(loc)));
    if let Some(date) = lastmod {
        out.push_str(&format!("<lastmod>{date}</lastmod>"));
    }
    out.push_str("</url>\n");
}

/// RFC 3339 timestamp at midnight UTC; invalid dates fall back to the epoch.
fn timestamp(date: &str) -> String {
    let date = if is_iso_date(date) {
        date
    } else {
        "1970-01-01"
    };
    format!("{date}T00:00:00Z")
}
