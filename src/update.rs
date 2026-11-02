//! `bare update`: merges the upstream bare repository into a fork, leaving out
//! the paths listed in `update_exclude` in site.conf. It stops before the
//! commit, so the result can be reviewed (or abandoned with `git merge --abort`).
//!
//! Everything goes through the `git` command, so bare itself stays std-only.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

/// Where forks get updates from when they have no `upstream` remote.
pub const UPSTREAM_URL: &str = "https://github.com/Nagatani/bare.git";
/// The upstream branch that gets merged.
const UPSTREAM_BRANCH: &str = "main";

/// What `bare update` did.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Upstream had nothing new; nothing was changed.
    UpToDate { source: String },
    /// A merge is in progress and waits for `git commit`.
    Merged {
        source: String,
        /// `git status --short` lines of what the commit would contain.
        changes: Vec<String>,
        /// Files left with conflicts, to be resolved by hand.
        conflicts: Vec<String>,
        /// The `update_exclude` paths, kept as they were before the merge.
        excluded: Vec<String>,
    },
}

/// Runs the update for the repository that contains the current directory,
/// reading `update_exclude` from `content_dir/site.conf`.
///
/// A relative `content_dir` is taken from the repository root rather than the
/// current directory: the exclude paths are root-relative too, and running
/// from a subdirectory must not silently lose them.
pub fn update(content_dir: &Path) -> Result<Outcome, String> {
    let root = PathBuf::from(
        git_text(Path::new("."), &["rev-parse", "--show-toplevel"])
            .map_err(|err| format!("not inside a git repository ({err})"))?,
    );
    let excluded = read_excludes(&root.join(content_dir).join("site.conf"))?;
    // An `upstream` remote, if the fork set one up, wins over the built-in URL,
    // so forks of forks can follow their own parent.
    let source = git_text(&root, &["remote", "get-url", "upstream"])
        .map(|_| "upstream".to_string())
        .unwrap_or_else(|_| UPSTREAM_URL.to_string());
    println!("bare: fetching {UPSTREAM_BRANCH} from {source}");
    update_from(&root, &source, &excluded)
}

/// The update itself, with the repository and the source made explicit so
/// tests can point it at local repositories.
fn update_from(root: &Path, source: &str, excluded: &[String]) -> Result<Outcome, String> {
    if git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])?
        .status
        .success()
    {
        return Err(
            "a merge is already in progress: finish it with `git commit` \
             or cancel it with `git merge --abort`"
                .to_string(),
        );
    }
    let dirty = git_text(root, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        return Err(format!(
            "commit or stash your changes before updating:\n{}",
            indent(&dirty)
        ));
    }

    git_text(root, &["fetch", "--quiet", source, UPSTREAM_BRANCH])?;
    if git(root, &["merge-base", "--is-ancestor", "FETCH_HEAD", "HEAD"])?
        .status
        .success()
    {
        return Ok(Outcome::UpToDate {
            source: source.to_string(),
        });
    }

    // A conflicting merge exits with an error but still leaves MERGE_HEAD,
    // which is the case handled below; anything else is a real failure.
    let merge = git(root, &["merge", "--no-commit", "--no-ff", "FETCH_HEAD"])?;
    if !git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])?
        .status
        .success()
    {
        return Err(format!(
            "git merge failed:\n{}",
            indent(String::from_utf8_lossy(&merge.stderr).trim())
        ));
    }

    if !excluded.is_empty() {
        restore_excluded(root, excluded)?;
    }

    let conflicts = lines(&git_text(
        root,
        &["diff", "--name-only", "--diff-filter=U"],
    )?);
    let changes = lines(&git_text(
        root,
        &["status", "--short", "--untracked-files=no"],
    )?);
    Ok(Outcome::Merged {
        source: source.to_string(),
        changes,
        conflicts,
        excluded: excluded.to_vec(),
    })
}

/// Puts the excluded paths back to their state in HEAD: whatever the merge
/// added, changed, deleted or left in conflict there is undone.
///
/// `git restore --source=HEAD` alone would refuse a path that exists only
/// upstream, so the merged versions are removed first and the paths that
/// exist in HEAD are checked out again.
fn restore_excluded(root: &Path, excluded: &[String]) -> Result<(), String> {
    let mut args = vec!["rm", "-r", "-f", "-q", "--ignore-unmatch", "--"];
    args.extend(excluded.iter().map(String::as_str));
    git_text(root, &args)?;
    for path in excluded {
        let in_head = git(root, &["cat-file", "-e", &format!("HEAD:{path}")])?
            .status
            .success();
        if in_head {
            git_text(root, &["checkout", "HEAD", "--", path])?;
        }
    }
    Ok(())
}

/// Reads `update_exclude` from site.conf. A missing file or key excludes nothing.
fn read_excludes(conf: &Path) -> Result<Vec<String>, String> {
    let text = match fs::read_to_string(conf) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("{}: {err}", conf.display())),
    };
    let fields = crate::parser::parse_key_values(&text);
    parse_excludes(fields.get("update_exclude").unwrap_or(""))
        .map_err(|message| format!("{}: update_exclude: {message}", conf.display()))
}

/// Splits `update_exclude` into repository-relative paths. Paths that could
/// reach outside the repository, or that git would read as pathspec magic,
/// are rejected rather than passed on.
fn parse_excludes(value: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for word in value.split_whitespace() {
        let path = word.trim_end_matches('/');
        let plain = !path.is_empty()
            && !path.starts_with(':')
            && !path.starts_with('-')
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        if !plain {
            return Err(format!(
                "`{word}` is not a path inside the repository, like content/posts"
            ));
        }
        if !paths.iter().any(|p| p == path) {
            paths.push(path.to_string());
        }
    }
    Ok(paths)
}

/// Runs git in `dir`. Only failing to start git is an error here; callers
/// decide what a non-zero exit means.
fn git(dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        // Never open an editor or prompt, even if the user's config asks for it.
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => "git not found: install git to use `bare update`".into(),
            _ => format!("could not run git: {err}"),
        })
}

/// Runs git and returns its trimmed stdout, or its stderr as the error.
fn git_text(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = git(dir, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    } else {
        Err(format!(
            "`git {}` failed:\n{}",
            args.join(" "),
            indent(String::from_utf8_lossy(&output.stderr).trim())
        ))
    }
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exclude_paths() {
        assert_eq!(
            parse_excludes("content/posts  content/pages/ content/posts").unwrap(),
            ["content/posts", "content/pages"]
        );
        assert!(parse_excludes("").unwrap().is_empty());
        for bad in [
            "../x",
            "/etc",
            ":(glob)*",
            "-x",
            "content/../src",
            "./content",
            "/",
        ] {
            assert!(parse_excludes(bad).is_err(), "{bad} should be rejected");
        }
    }

    /// Runs git for test setup, with an identity and no signing so the user's
    /// global config cannot get in the way.
    fn run(dir: &Path, args: &[&str]) {
        let mut full = vec![
            "-c",
            "user.name=bare",
            "-c",
            "user.email=bare@example.com",
            "-c",
            "commit.gpgsign=false",
        ];
        full.extend_from_slice(args);
        let output = git(dir, &full).unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn merges_upstream_except_excluded_paths() {
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("skipped: git not found");
            return;
        }
        let base = std::env::temp_dir().join(format!("bare-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let up = base.join("up");
        let fork = base.join("fork");

        fs::create_dir_all(&up).unwrap();
        run(&up, &["init", "-q", "-b", "main"]);
        write(&up.join("src/main.rs"), "v1\n");
        write(&up.join("content/posts/sample.md"), "sample v1\n");
        write(&up.join("content/posts/old.md"), "old\n");
        run(&up, &["add", "-A"]);
        run(&up, &["commit", "-q", "-m", "init"]);
        run(&base, &["clone", "-q", "up", "fork"]);

        // The fork edits a sample post and writes its own.
        write(&fork.join("content/posts/sample.md"), "mine\n");
        write(&fork.join("content/posts/my-post.md"), "my post\n");
        run(&fork, &["add", "-A"]);
        run(&fork, &["commit", "-q", "-m", "mine"]);

        let source = up.to_str().unwrap();
        let excluded = parse_excludes("content/posts content/pages").unwrap();
        assert_eq!(
            update_from(&fork, source, &excluded).unwrap(),
            Outcome::UpToDate {
                source: source.into()
            }
        );

        // Upstream changes the engine, and adds, edits (in conflict) and
        // deletes posts, plus a directory the fork never had.
        write(&up.join("src/main.rs"), "v2\n");
        write(&up.join("content/posts/sample.md"), "sample v2\n");
        write(&up.join("content/posts/new.md"), "new\n");
        write(&up.join("content/pages/about.md"), "about\n");
        fs::remove_file(up.join("content/posts/old.md")).unwrap();
        run(&up, &["add", "-A"]);
        run(&up, &["commit", "-q", "-m", "update"]);

        let outcome = update_from(&fork, source, &excluded).unwrap();
        assert_eq!(
            outcome,
            Outcome::Merged {
                source: source.into(),
                changes: vec!["M  src/main.rs".into()],
                conflicts: vec![],
                excluded: excluded.clone(),
            }
        );
        let read = |p: &str| fs::read_to_string(fork.join(p)).unwrap();
        assert_eq!(read("src/main.rs"), "v2\n");
        assert_eq!(read("content/posts/sample.md"), "mine\n");
        assert_eq!(read("content/posts/old.md"), "old\n");
        assert_eq!(read("content/posts/my-post.md"), "my post\n");
        assert!(!fork.join("content/posts/new.md").exists());
        assert!(!fork.join("content/pages").exists());

        // A second run refuses to start while that merge is pending.
        assert!(update_from(&fork, source, &excluded)
            .unwrap_err()
            .contains("already in progress"));
        run(&fork, &["merge", "--abort"]);

        // Without exclusions the sample post conflicts and is reported.
        match update_from(&fork, source, &[]).unwrap() {
            Outcome::Merged { conflicts, .. } => {
                assert_eq!(conflicts, ["content/posts/sample.md"]);
            }
            other => panic!("unexpected {other:?}"),
        }
        run(&fork, &["merge", "--abort"]);

        // Uncommitted changes stop the update before anything is fetched.
        write(&fork.join("src/main.rs"), "local edit\n");
        assert!(update_from(&fork, source, &excluded)
            .unwrap_err()
            .contains("commit or stash"));

        fs::remove_dir_all(&base).unwrap();
    }
}
