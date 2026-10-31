//! bare — a zero-dependency, minimalist static site generator.

mod parser;
mod site;
mod template;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use site::Config;

const USAGE: &str = "\
bare — a zero-dependency static site generator

USAGE:
    bare [build] [OPTIONS]

OPTIONS:
    -c, --content <DIR>   Source directory with posts/ and templates/  [default: content]
    -o, --out <DIR>       Output directory                             [default: dist]
        --clean           Delete the whole output directory before building
    -h, --help            Print this help
    -V, --version         Print version";

enum Command {
    Build(Config),
    Help,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut config = Config {
        content_dir: PathBuf::from("content"),
        out_dir: PathBuf::from("dist"),
        clean: false,
    };
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "build" => {}
            "-h" | "--help" | "help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--clean" => config.clean = true,
            "-c" | "--content" | "-o" | "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for `{arg}`"))?;
                if matches!(arg.as_str(), "-c" | "--content") {
                    config.content_dir = PathBuf::from(value);
                } else {
                    config.out_dir = PathBuf::from(value);
                }
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Command::Build(config))
}

fn main() -> ExitCode {
    let config = match parse_args(env::args().skip(1)) {
        Ok(Command::Build(config)) => config,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            println!("bare {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    println!(
        "bare: building {} -> {}{}",
        config.content_dir.display(),
        config.out_dir.display(),
        if config.clean { " (clean)" } else { "" }
    );
    let started = Instant::now();

    match site::build(&config) {
        Ok(report) => {
            for path in &report.written {
                println!("  wrote {}", path.display());
            }
            println!(
                "bare: generated {} files ({} posts + index), copied {} static files in {} ms",
                report.written.len(),
                report.posts,
                report.static_files,
                started.elapsed().as_millis()
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
