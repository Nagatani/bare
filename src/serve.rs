//! `bare serve`: a tiny development server with rebuild-on-change and live reload.
//!
//! It mimics GitHub Pages so what works locally works when published: the site
//! lives under the same sub-path (e.g. `/bare/`), `/posts/a` resolves to
//! `posts/a.html`, directories redirect to a trailing slash and unknown URLs get
//! `404.html` with status 404. A small polling script is injected into HTML
//! responses only; the files written to disk stay JavaScript-free.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use crate::site::Config;

/// Endpoint the injected script polls; its body changes after every rebuild.
const VERSION_PATH: &str = "/__bare/version";

const RELOAD_SCRIPT: &str = "<script>(() => {\
let seen = null;\
const tick = async () => {\
try {\
const res = await fetch(\"/__bare/version\", { cache: \"no-store\" });\
const now = await res.text();\
if (seen !== null && now !== seen) { location.reload(); return; }\
seen = now;\
} catch (_) {}\
setTimeout(tick, 500);\
};\
tick();\
})();</script>";

pub struct ServeOptions {
    pub port: u16,
    /// URL path the site is served under; detected like GitHub Pages when `None`.
    pub subpath: Option<String>,
}

/// Builds the site, serves it and rebuilds whenever `config.content_dir` changes.
/// `build` performs one build and reports whether it succeeded.
pub fn serve(
    mut config: Config,
    options: ServeOptions,
    build: fn(&Config) -> bool,
) -> io::Result<()> {
    let subpath = normalize_subpath(
        &options
            .subpath
            .unwrap_or_else(|| detect_subpath(&config.content_dir)),
    );
    let addr = format!("127.0.0.1:{}", options.port);
    let listener = TcpListener::bind(&addr)?;
    config.base_url = Some(format!("http://{addr}{subpath}"));
    config.clean = true;

    let version = Arc::new(AtomicU64::new(0));
    if build(&config) {
        version.fetch_add(1, Ordering::SeqCst);
    }
    println!("bare: serving http://{addr}{subpath}  (drafts included, Ctrl+C to stop)");

    {
        let config = config.clone();
        let version = Arc::clone(&version);
        thread::spawn(move || watch(&config, &version, build));
    }

    let out_dir = Arc::new(config.out_dir.clone());
    let subpath = Arc::new(subpath);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let out_dir = Arc::clone(&out_dir);
        let subpath = Arc::clone(&subpath);
        let version = Arc::clone(&version);
        thread::spawn(move || {
            // A client hanging up mid-response is not worth reporting.
            let _ = handle(stream, &out_dir, &subpath, &version);
        });
    }
    Ok(())
}

/// Polls the content directory and rebuilds when anything changes.
fn watch(config: &Config, version: &AtomicU64, build: fn(&Config) -> bool) {
    let mut last = fingerprint(&config.content_dir);
    loop {
        thread::sleep(Duration::from_millis(300));
        let now = fingerprint(&config.content_dir);
        if now != last {
            last = now;
            if build(config) {
                version.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

/// Order-independent summary of every file's path length, size and mtime.
fn fingerprint(dir: &Path) -> u64 {
    let mut hash = 0u64;
    let Ok(entries) = fs::read_dir(dir) else {
        return hash;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        let value = if meta.is_dir() {
            fingerprint(&path)
        } else {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos() as u64);
            mtime ^ meta.len().rotate_left(32) ^ (path.as_os_str().len() as u64).rotate_left(48)
        };
        // Summing keeps the result independent of directory listing order.
        hash = hash
            .wrapping_add(value.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(1);
    }
    hash
}

fn handle(stream: TcpStream, out_dir: &Path, subpath: &str, version: &AtomicU64) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Drain the headers; nothing in them matters here.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let head_only = method == "HEAD";
    let mut stream = stream;
    if method != "GET" && !head_only {
        return respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"method not allowed",
            head_only,
        );
    }

    let path = target.split(['?', '#']).next().unwrap_or("/");
    if path == VERSION_PATH {
        let body = version.load(Ordering::SeqCst).to_string();
        return respond(
            &mut stream,
            "200 OK",
            "text/plain",
            body.as_bytes(),
            head_only,
        );
    }

    match resolve(out_dir, subpath, path) {
        Resolved::File(file) => match fs::read(&file) {
            Ok(body) => send_file(&mut stream, "200 OK", &file, body, head_only),
            Err(_) => not_found(&mut stream, out_dir, head_only),
        },
        Resolved::Redirect(location) => {
            let header = format!(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(header.as_bytes())
        }
        Resolved::NotFound => not_found(&mut stream, out_dir, head_only),
    }
}

#[derive(Debug, PartialEq)]
enum Resolved {
    File(PathBuf),
    Redirect(String),
    NotFound,
}

/// Maps a URL path to a file the way GitHub Pages does.
fn resolve(out_dir: &Path, subpath: &str, path: &str) -> Resolved {
    if path == subpath.trim_end_matches('/') || (path == "/" && subpath != "/") {
        return Resolved::Redirect(subpath.to_string());
    }
    let Some(rest) = path.strip_prefix(subpath) else {
        return Resolved::NotFound;
    };
    let Some(rest) = percent_decode(rest) else {
        return Resolved::NotFound;
    };
    if rest
        .split('/')
        .any(|segment| segment == ".." || segment.contains('\\'))
    {
        return Resolved::NotFound;
    }

    let candidate = out_dir.join(&rest);
    if rest.is_empty() || rest.ends_with('/') {
        let index = candidate.join("index.html");
        return if index.is_file() {
            Resolved::File(index)
        } else {
            Resolved::NotFound
        };
    }
    if candidate.is_dir() {
        return Resolved::Redirect(format!("{path}/"));
    }
    if candidate.is_file() {
        return Resolved::File(candidate);
    }
    // `/posts/a` → `posts/a.html`
    let html = out_dir.join(format!("{rest}.html"));
    if html.is_file() {
        Resolved::File(html)
    } else {
        Resolved::NotFound
    }
}

fn not_found(stream: &mut TcpStream, out_dir: &Path, head_only: bool) -> io::Result<()> {
    let page = out_dir.join("404.html");
    match fs::read(&page) {
        Ok(body) => send_file(stream, "404 Not Found", &page, body, head_only),
        Err(_) => respond(
            stream,
            "404 Not Found",
            "text/plain",
            b"404 not found",
            head_only,
        ),
    }
}

fn send_file(
    stream: &mut TcpStream,
    status: &str,
    file: &Path,
    mut body: Vec<u8>,
    head_only: bool,
) -> io::Result<()> {
    let content_type = content_type(file);
    if content_type.starts_with("text/html") {
        body = inject_reload_script(body);
    }
    respond(stream, status, content_type, &body, head_only)
}

fn inject_reload_script(body: Vec<u8>) -> Vec<u8> {
    let mut html = match String::from_utf8(body) {
        Ok(html) => html,
        Err(err) => return err.into_bytes(),
    };
    match html.rfind("</body>") {
        Some(at) => html.insert_str(at, RELOAD_SCRIPT),
        None => html.push_str(RELOAD_SCRIPT),
    }
    html.into_bytes()
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn content_type(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "json" => "application/json",
        "txt" | "md" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Decodes `%XX` escapes; `None` for malformed input or invalid UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Guesses the GitHub Pages sub-path: `/` with a custom domain (`static/CNAME`)
/// or for a `<user>.github.io` repository, otherwise `/<repository>/` taken from
/// the `origin` remote in `.git/config`.
fn detect_subpath(content_dir: &Path) -> String {
    if content_dir.join("static/CNAME").is_file() {
        return "/".to_string();
    }
    let Ok(git_config) = fs::read_to_string(".git/config") else {
        return "/".to_string();
    };
    let mut in_origin = false;
    for line in git_config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = line == "[remote \"origin\"]";
        } else if let Some(url) = line.strip_prefix("url").filter(|_| in_origin) {
            let url = url.trim_start().trim_start_matches('=').trim();
            let name = url
                .trim_end_matches('/')
                .rsplit(['/', ':'])
                .next()
                .unwrap_or("")
                .trim_end_matches(".git");
            if name.is_empty() || name.ends_with(".github.io") {
                return "/".to_string();
            }
            return format!("/{name}/");
        }
    }
    "/".to_string()
}

/// Ensures a leading and trailing slash: `bare` → `/bare/`, `` → `/`.
fn normalize_subpath(path: &str) -> String {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed}/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_subpaths() {
        assert_eq!(normalize_subpath("bare"), "/bare/");
        assert_eq!(normalize_subpath("/bare/"), "/bare/");
        assert_eq!(normalize_subpath(""), "/");
        assert_eq!(normalize_subpath("/"), "/");
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("a%20b").as_deref(), Some("a b"));
        assert_eq!(percent_decode("%E6%97%A5").as_deref(), Some("日"));
        assert_eq!(percent_decode("%zz"), None);
    }

    #[test]
    fn resolves_like_github_pages() {
        let dir = std::env::temp_dir().join(format!("bare-serve-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("posts")).unwrap();
        fs::write(dir.join("index.html"), "i").unwrap();
        fs::write(dir.join("posts/a.html"), "a").unwrap();

        assert_eq!(
            resolve(&dir, "/bare/", "/"),
            Resolved::Redirect("/bare/".into())
        );
        assert_eq!(
            resolve(&dir, "/bare/", "/bare"),
            Resolved::Redirect("/bare/".into())
        );
        assert_eq!(
            resolve(&dir, "/bare/", "/bare/"),
            Resolved::File(dir.join("index.html"))
        );
        assert_eq!(
            resolve(&dir, "/bare/", "/bare/posts/a"),
            Resolved::File(dir.join("posts/a.html"))
        );
        assert_eq!(
            resolve(&dir, "/bare/", "/bare/posts"),
            Resolved::Redirect("/bare/posts/".into())
        );
        assert_eq!(
            resolve(&dir, "/bare/", "/bare/../secret"),
            Resolved::NotFound
        );
        assert_eq!(resolve(&dir, "/bare/", "/other/"), Resolved::NotFound);

        fs::remove_dir_all(&dir).unwrap();
    }
}
