//! OGP images: each post's card (the theme's `og.html`, filled in per post) is
//! rendered to a 1200×630 PNG by a headless Chrome.
//!
//! bare has no crate dependencies and cannot rasterise text itself (Japanese
//! alone needs thousands of glyphs), so it borrows a browser that is usually
//! already there: GitHub's Ubuntu runners ship Google Chrome. Without Chrome the
//! images are skipped and pages fall back to the site-wide `image`.
//!
//! Rendering is cached by the card's contents in `target/bare-og/`, which the
//! Pages workflow keeps between runs with `actions/cache`, so only posts whose
//! card changed (title, date, description, theme) launch Chrome again.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// Size of the generated images, the size OGP consumers expect for large cards.
pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 630;

/// Where rendered images are kept between builds, relative to the working
/// directory (next to `target/bare-serve/`, and inside the cached `target/`).
const CACHE_DIR: &str = "target/bare-og";

/// Chrome processes run at once. Each launch takes about a second, mostly
/// start-up, so a few in parallel cut the first build noticeably.
const WORKERS: usize = 4;

/// A Chrome that hangs (e.g. on a font it cannot load) is killed after this.
const TIMEOUT: Duration = Duration::from_secs(30);

/// One image to produce: the card's HTML and the PNG file to write.
pub struct Card {
    pub html: String,
    pub target: PathBuf,
}

/// What [`render`] did, for the build report.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Targets that now exist; cards missing here failed or were skipped.
    pub written: Vec<PathBuf>,
    /// How many of those came from the cache without launching Chrome.
    pub cached: usize,
    pub warnings: Vec<String>,
}

/// Writes every card's PNG, from the cache when possible and otherwise by
/// running `chrome`. With no Chrome, only cached images are written.
pub fn render(cards: &[Card], chrome: Option<&Path>) -> Outcome {
    render_in(cards, chrome, Path::new(CACHE_DIR))
}

fn render_in(cards: &[Card], chrome: Option<&Path>, cache: &Path) -> Outcome {
    let mut outcome = Outcome::default();
    if let Err(err) = fs::create_dir_all(cache) {
        outcome.warnings.push(format!(
            "{}: cannot create OGP cache: {err}",
            cache.display()
        ));
        return outcome;
    }

    // Cache hits first; the rest are rendered below.
    let mut pending = Vec::new();
    for card in cards {
        let cached = cache.join(format!("{}.png", hash(&card.html)));
        if is_png(&cached) {
            match copy_to(&cached, &card.target) {
                Ok(()) => {
                    outcome.written.push(card.target.clone());
                    outcome.cached += 1;
                }
                Err(err) => outcome
                    .warnings
                    .push(format!("{}: {err}", card.target.display())),
            }
        } else {
            pending.push((card, cached));
        }
    }
    let Some(chrome) = chrome else {
        prune(cache, cards);
        return outcome;
    };

    // A shared counter hands out the next card to whichever worker is free.
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for worker in 0..WORKERS.min(pending.len()) {
            let (next, results, pending) = (&next, &results, &pending);
            scope.spawn(move || {
                // Chrome refuses to share a profile between running instances.
                let profile = cache.join(format!("profile-{worker}"));
                while let Some((card, cached)) = pending.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let result = screenshot(chrome, &profile, &card.html, cached)
                        .and_then(|()| copy_to(cached, &card.target));
                    results.lock().unwrap().push((card.target.clone(), result));
                }
            });
        }
    });
    prune(cache, cards);
    for (target, result) in results.into_inner().unwrap() {
        match result {
            Ok(()) => outcome.written.push(target),
            Err(err) => outcome.warnings.push(format!(
                "{}: OGP image not generated: {err}",
                target.display()
            )),
        }
    }
    outcome.written.sort();
    outcome
}

/// Renders `html` to `png` with Chrome, through a temporary file next to it.
fn screenshot(chrome: &Path, profile: &Path, html: &str, png: &Path) -> io::Result<()> {
    let page = png.with_extension("html");
    fs::write(&page, html)?;
    let page_url = file_url(&fs::canonicalize(&page)?);
    let mut child = Command::new(chrome)
        .args([
            "--headless=new",
            // The page is our own local file, so the sandbox adds nothing, and
            // it fails to start on Ubuntu runners that restrict user namespaces.
            "--no-sandbox",
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            "--force-device-scale-factor=1",
        ])
        .arg(format!("--window-size={WIDTH},{HEIGHT}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--screenshot={}", png.display()))
        .arg(page_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // Chrome logs harmless errors on every run; keep the build output clean.
        .stderr(Stdio::null())
        .spawn()?;

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&page);
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Chrome timed out"));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let _ = fs::remove_file(&page);
    if !status.success() || !is_png(png) {
        // A half-written file must not be served from the cache next time.
        let _ = fs::remove_file(png);
        return Err(io::Error::other(format!("Chrome failed ({status})")));
    }
    Ok(())
}

/// The Chrome to use: `BARE_CHROME` if set, else a known name on `PATH`, else
/// the usual macOS install location. `None` when there is none; a
/// `BARE_CHROME` that is not a file also gives `None`, so the build reports
/// "Chrome not found" once instead of failing for every post.
pub fn find_chrome() -> Option<PathBuf> {
    if let Some(path) = env::var_os("BARE_CHROME").filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    const NAMES: [&str; 5] = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ];
    let on_path = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .flat_map(|dir| NAMES.iter().map(move |name| dir.join(name)))
            .find(|path| path.is_file())
    });
    on_path.or_else(|| {
        let mac = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        mac.is_file().then_some(mac)
    })
}

/// Removes cached images no current card uses (old titles, removed posts), so
/// the cache, which CI stores between runs, does not grow without bound.
/// Chrome profiles (`profile-*` directories) are kept to speed up start-up.
fn prune(cache: &Path, cards: &[Card]) {
    let keep: Vec<String> = cards
        .iter()
        .map(|card| format!("{}.png", hash(&card.html)))
        .collect();
    let Ok(entries) = fs::read_dir(cache) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stale_png = name.ends_with(".png") && !keep.contains(&name);
        // A card page left behind by a killed build.
        let leftover_page = name.ends_with(".html");
        if stale_png || leftover_page {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn copy_to(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::copy(from, to).map(|_| ())
}

/// True if `path` is a non-empty file starting with the PNG signature.
fn is_png(path: &Path) -> bool {
    let mut header = [0u8; 8];
    fs::File::open(path)
        .and_then(|mut file| io::Read::read_exact(&mut file, &mut header))
        .is_ok()
        && header == *b"\x89PNG\r\n\x1a\n"
}

/// `file://` URL for an absolute path, percent-encoding anything but safe
/// characters and `/` (paths may contain spaces or Japanese).
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// 64-bit FNV-1a of the card, as the cache file name. The image size and this
/// module's flags are part of the key so changing them invalidates old images.
fn hash(html: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let key = format!("{WIDTH}x{HEIGHT}\n{html}");
    for &byte in key.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_file_urls() {
        assert_eq!(
            file_url(Path::new("/tmp/my dir/カード.html")),
            "file:///tmp/my%20dir/%E3%82%AB%E3%83%BC%E3%83%89.html"
        );
    }

    #[test]
    fn hashes_depend_on_the_card() {
        assert_eq!(hash("a"), hash("a"));
        assert_ne!(hash("a"), hash("b"));
        assert_eq!(hash("a").len(), 16);
    }

    #[test]
    fn serves_cached_images_without_chrome() {
        let dir = env::temp_dir().join(format!("bare-og-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let png = b"\x89PNG\r\n\x1a\nrest";
        fs::write(cache.join(format!("{}.png", hash("<p>cached</p>"))), png).unwrap();
        // Not a PNG: must be ignored rather than copied.
        fs::write(
            cache.join(format!("{}.png", hash("<p>broken</p>"))),
            b"oops",
        )
        .unwrap();

        let cards = [
            Card {
                html: "<p>cached</p>".into(),
                target: dir.join("out/og/a.png"),
            },
            Card {
                html: "<p>broken</p>".into(),
                target: dir.join("out/og/b.png"),
            },
        ];
        let outcome = render_in(&cards, None, &cache);
        assert_eq!(outcome.written, [dir.join("out/og/a.png")]);
        assert_eq!(outcome.cached, 1);
        assert_eq!(fs::read(dir.join("out/og/a.png")).unwrap(), png);
        assert!(!dir.join("out/og/b.png").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Writes an executable shell script standing in for Chrome.
    #[cfg(unix)]
    fn script(path: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        path.to_path_buf()
    }

    #[cfg(unix)]
    #[test]
    fn renders_with_chrome_and_prunes_the_cache() {
        let dir = env::temp_dir().join(format!("bare-og-chrome-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        // Writes a minimal PNG to the path given as --screenshot=.
        let chrome = script(
            &dir.join("fake-chrome"),
            "for a in \"$@\"; do case \"$a\" in --screenshot=*) out=\"${a#--screenshot=}\";; esac; done\n\
             printf '\\211PNG\\r\\n\\032\\nfake' > \"$out\"",
        );
        let failing = script(&dir.join("failing-chrome"), "exit 1");
        fs::write(cache.join("0000000000000000.png"), b"\x89PNG\r\n\x1a\nold").unwrap();

        let cards: Vec<Card> = ["a", "b"]
            .iter()
            .map(|name| Card {
                html: format!("<p>{name}</p>"),
                target: dir.join(format!("out/{name}.png")),
            })
            .collect();
        let outcome = render_in(&cards, Some(&chrome), &cache);
        assert_eq!(outcome.written.len(), 2, "{:?}", outcome.warnings);
        assert_eq!(outcome.cached, 0);
        assert!(is_png(&dir.join("out/a.png")));
        assert!(
            !cache.join("0000000000000000.png").exists(),
            "stale image pruned"
        );

        // Second run: everything comes from the cache, even with a broken Chrome.
        let outcome = render_in(&cards, Some(&failing), &cache);
        assert_eq!((outcome.written.len(), outcome.cached), (2, 2));

        // A failing Chrome gives a warning and leaves nothing in the cache.
        let new = [Card {
            html: "<p>new</p>".into(),
            target: dir.join("out/new.png"),
        }];
        let outcome = render_in(&new, Some(&failing), &cache);
        assert!(outcome.written.is_empty());
        assert_eq!(outcome.warnings.len(), 1);
        assert!(!cache.join(format!("{}.png", hash("<p>new</p>"))).exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
