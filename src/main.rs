//! bare — a zero-dependency, minimalist static site generator.

mod assets;
mod check;
mod feed;
mod highlight;
mod images;
mod parser;
mod scaffold;
mod serve;
mod site;
mod template;
mod text;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use check::Severity;
use scaffold::NewOptions;
use serve::ServeOptions;
use site::Config;

const USAGE: &str = "\
bare — a zero-dependency static site generator

USAGE:
    bare [build] [OPTIONS]   Build the site into the output directory
    bare check [OPTIONS]     Build, then fail on broken links or anchors
    bare serve [OPTIONS]     Preview with rebuild on change and live reload
    bare new <NAME> [--title <TITLE>] [--page]
                             Create content/posts/NAME.md (a draft) or pages/NAME.md

OPTIONS:
    -c, --content <DIR>   Source directory with posts/ and templates/  [default: content]
    -o, --out <DIR>       Output directory          [default: dist, serve: target/bare-serve]
        --clean           Delete the whole output directory before building
        --drafts          Include posts marked `draft: true` (always on for serve)
        --base-url <URL>  Public site URL for feed, sitemap and OGP
                          [default: base_url in site.conf]
    -p, --port <PORT>     Port for serve                               [default: 3000]
        --subpath <PATH>  URL path for serve, e.g. /bare/  [default: detected like GitHub Pages]
        --title <TITLE>   Title for new                                [default: NAME]
        --page            Make new create a standalone page instead of a post
    -h, --help            Print this help
    -V, --version         Print version";

enum Command {
    Build(Config),
    Check(Config),
    Serve(Config, ServeOptions),
    New(Config, NewOptions),
    Help,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut config = Config {
        content_dir: PathBuf::from("content"),
        out_dir: PathBuf::from("dist"),
        clean: false,
        drafts: false,
        base_url: None,
    };
    let mut serve = ServeOptions {
        port: 3000,
        subpath: None,
    };
    let mut new = NewOptions {
        slug: String::new(),
        title: None,
        page: false,
    };
    let mut mode = "build";
    let mut out_given = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "build" => mode = "build",
            "check" => mode = "check",
            "serve" => mode = "serve",
            "new" if mode == "build" => mode = "new",
            "--page" => new.page = true,
            name if mode == "new" && new.slug.is_empty() && !name.starts_with('-') => {
                new.slug = name.to_string();
            }
            "-h" | "--help" | "help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--clean" => config.clean = true,
            "--drafts" => config.drafts = true,
            "-c" | "--content" | "-o" | "--out" | "--base-url" | "-p" | "--port" | "--subpath"
            | "--title" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for `{arg}`"))?;
                match arg.as_str() {
                    "-c" | "--content" => config.content_dir = PathBuf::from(value),
                    "-o" | "--out" => {
                        config.out_dir = PathBuf::from(value);
                        out_given = true;
                    }
                    "-p" | "--port" => {
                        serve.port = value
                            .parse()
                            .map_err(|_| format!("invalid port `{value}`"))?;
                    }
                    "--subpath" => serve.subpath = Some(value),
                    "--title" => new.title = Some(value),
                    _ => config.base_url = Some(value),
                }
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }

    Ok(match mode {
        "check" => Command::Check(config),
        "new" if new.slug.is_empty() => return Err("missing NAME for `new`".to_string()),
        "new" => Command::New(config, new),
        "serve" => {
            // Keep preview builds (with localhost URLs) out of the publishable dist/.
            if !out_given {
                config.out_dir = PathBuf::from("target/bare-serve");
            }
            config.drafts = true;
            Command::Serve(config, serve)
        }
        _ => Command::Build(config),
    })
}

fn main() -> ExitCode {
    let command = match parse_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Help => println!("{USAGE}"),
        Command::Version => println!("bare {}", env!("CARGO_PKG_VERSION")),
        Command::Build(config) => {
            if build(&config, true).is_none() {
                return ExitCode::FAILURE;
            }
        }
        Command::Check(config) => match build(&config, true) {
            Some(errors) if errors > 0 => {
                eprintln!("bare check: {errors} error(s) found");
                return ExitCode::FAILURE;
            }
            Some(_) => println!("bare check: no broken links"),
            None => return ExitCode::FAILURE,
        },
        Command::New(config, options) => match scaffold::create(&config.content_dir, &options) {
            Ok(path) => {
                println!("bare: created {}", path.display());
                if !options.page {
                    println!("  it is a draft: preview with `bare serve`, remove `draft: true` to publish");
                }
            }
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        },
        Command::Serve(config, options) => {
            if let Err(err) = serve::serve(config, options, |config| build(config, false).is_some())
            {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Builds the site, checks its links and prints a report. Returns the number of
/// link errors, or `None` if the build itself failed.
fn build(config: &Config, verbose: bool) -> Option<usize> {
    if verbose {
        println!(
            "bare: building {} -> {}{}",
            config.content_dir.display(),
            config.out_dir.display(),
            if config.clean { " (clean)" } else { "" }
        );
    }
    let started = Instant::now();

    let report = match site::build(config) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("error: {err}");
            return None;
        }
    };
    let problems = check::check_site(&config.out_dir, &report.base_path);

    if verbose {
        for path in &report.written {
            println!("  wrote {}", path.display());
        }
        for note in &report.notes {
            println!("  note: {note}");
        }
    }
    for warning in &report.warnings {
        eprintln!("  warning: {warning}");
    }
    for problem in &problems {
        let label = match problem.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        eprintln!("  {label}: {problem}");
    }

    let drafts = match report.drafts_skipped {
        0 => String::new(),
        n => format!(", skipped {n} drafts"),
    };
    println!(
        "bare: generated {} files ({} posts, {} pages){drafts}, copied {} static files in {} ms",
        report.written.len(),
        report.posts,
        report.pages,
        report.static_files,
        started.elapsed().as_millis()
    );
    Some(
        problems
            .iter()
            .filter(|p| p.severity == Severity::Error)
            .count(),
    )
}
