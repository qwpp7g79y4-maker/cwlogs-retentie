use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[allow(unsafe_code)]
mod interrupt;
#[allow(unsafe_code)]
mod stdout_probe;

use cwlogs_retentie::{
    escape_controls, escape_os_str, execute_paths, load_prices_file, Error, Options,
};

/// `--version` text. A debug build adds the commit that build.rs found.
#[cfg(all(debug_assertions, cwlogs_retentie_git_sha))]
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CWLOGS_RETENTIE_GIT_SHA"),
    ")"
);
#[cfg(not(all(debug_assertions, cwlogs_retentie_git_sha)))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estimate CloudWatch Logs storage cost from describe-log-groups JSON. Reads files only, no network.
#[derive(Parser)]
#[command(
    name = "cwlogs-retentie",
    bin_name = "cwlogs-retentie",
    version = VERSION,
    about = "Estimate CloudWatch Logs storage cost from describe-log-groups JSON. Reads files only, no network.",
    after_help = "Exit codes: 0 ok, 1 unretained group above --fail-stored-bytes, 2 input or usage error, 3 --quiet with --verbose or --json with --md."
)]
struct Cli {
    /// `describe-log-groups` JSON files, gzip exports, directories, or `-` for standard input.
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,
    /// Price file. The default is the built-in CloudWatch Logs storage list.
    #[arg(long, value_name = "FILE")]
    prices: Option<PathBuf>,
    /// Date ages are measured to, `YYYY-MM-DD`, midnight UTC. Without it, savings are omitted.
    #[arg(long, value_name = "YYYY-MM-DD")]
    as_of: Option<String>,
    /// Exit 1 when a group with no retention has storedBytes greater than this.
    #[arg(long, value_name = "BYTES", value_parser = parse_bytes)]
    fail_stored_bytes: Option<u64>,
    /// Print JSON instead of the text table.
    #[arg(long)]
    json: bool,
    /// Print a Markdown table instead of the text table.
    #[arg(long)]
    md: bool,
    /// Write the report to this file instead of standard output. An existing file is replaced.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Print only the result and errors: no notes, no warnings.
    #[arg(short, long)]
    quiet: bool,
    /// Add progress lines on standard error.
    #[arg(short, long)]
    verbose: bool,
}

fn parse_bytes(raw: &str) -> Result<u64, String> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("`{raw}` is not a plain non-negative integer"));
    }
    raw.parse::<u64>().map_err(|e| e.to_string())
}

/// Print a clap message with user-supplied control and direction characters escaped.
/// Arguments are escaped first and parsed again, so a newline inside a value cannot be
/// told apart from clap's own line breaks only after the fact.
/// Returns false when the message could not be written.
fn print_usage_error(err: &clap::Error) -> bool {
    let args: Vec<std::ffi::OsString> = std::env::args_os()
        .map(|arg| escape_os_str(&arg).into())
        .collect();
    let rendered = match Cli::try_parse_from(args) {
        Err(clean) => clean.render().to_string(),
        Ok(_) => err.render().to_string(),
    };
    // Clap's own line breaks stay; any other control or direction character is escaped.
    let safe: String = rendered
        .split('\n')
        .map(escape_controls)
        .collect::<Vec<_>>()
        .join("\n");
    if err.use_stderr() {
        let mut out = std::io::stderr();
        write!(out, "{safe}").is_ok() && out.flush().is_ok()
    } else {
        let mut out = stdout().lock();
        write!(out, "{safe}").is_ok() && out.flush().is_ok()
    }
}

fn main() -> ExitCode {
    interrupt::install();
    // Rust replaces a closed descriptor 1 with /dev/null before `main`; the probe
    // recorded before that whether it was closed.
    let stdout_closed = stdout_probe::stdout_was_closed_at_start();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let code = if err.use_stderr() { 2 } else { 0 };
            // A closed standard output still gets the usage error on standard error;
            // help and version have nowhere to go.
            if stdout_closed && code == 0 {
                return cannot_write();
            }
            if !print_usage_error(&err) && code == 0 {
                return cannot_write();
            }
            return ExitCode::from(code);
        }
    };
    if cli.quiet && cli.verbose {
        let _ = writeln!(
            std::io::stderr(),
            "error: --quiet and --verbose cannot be used together"
        );
        return ExitCode::from(3);
    }
    if cli.json && cli.md {
        let _ = writeln!(
            std::io::stderr(),
            "error: --json and --md cannot be used together"
        );
        return ExitCode::from(3);
    }
    match run(&cli) {
        Ok(output) => {
            if stdout_closed && cli.output.is_none() {
                return cannot_write();
            }
            if let Some(warning) = output.price_age_warning.as_ref().filter(|_| !cli.quiet) {
                let _ = writeln!(std::io::stderr(), "warning: {warning}");
            }
            if cli.json && !cli.quiet {
                for warning in &output.warnings {
                    let _ = writeln!(std::io::stderr(), "warning: {warning}");
                }
            }
            interrupt::set_writing(true);
            let written = match &cli.output {
                Some(path) => match write_report_file(path, output.stdout.as_bytes()) {
                    Ok(()) => true,
                    Err(err) => {
                        interrupt::set_writing(false);
                        let _ = writeln!(
                            std::io::stderr(),
                            "cwlogs-retentie: could not write the report to {}: {}",
                            escape_os_str(path.as_os_str()),
                            escape_controls(&err.to_string())
                        );
                        return ExitCode::from(2);
                    }
                },
                None => {
                    let mut out = stdout().lock();
                    out.write_all(output.stdout.as_bytes()).is_ok() && out.flush().is_ok()
                }
            };
            interrupt::set_writing(false);
            if !written {
                return cannot_write();
            }
            if output.threshold_exceeded {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "cwlogs-retentie: {err}");
            ExitCode::from(2)
        }
    }
}

/// Write `bytes` to a new temporary file in the directory of `path`, then rename it over
/// `path`. The temporary file is created exclusively, so a symlink is never followed; the
/// rename replaces a link at `path` itself and never its target. The temporary file is
/// removed when anything fails, and `path` is left as it was. A target that is a special
/// file or a directory is refused before anything is written.
fn write_report_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut name = std::ffi::OsString::from(".");
    name.push(
        path.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("report")),
    );
    name.push(format!(".cwlogs-retentie-{}.tmp", std::process::id()));
    let temp = parent.join(name);
    // A link at `path` is replaced, but a special file (pipe, device, socket) or a
    // directory, or a link pointing at one, is refused and left as it is.
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() && !meta.file_type().is_symlink() => {
            return Err(std::io::Error::other("not a regular file"));
        }
        // A link is replaced only when it points at a regular file or at nothing.
        Ok(_) => match std::fs::metadata(path) {
            Ok(target) if !target.is_file() => {
                return Err(std::io::Error::other("not a regular file"));
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn cannot_write() -> ExitCode {
    let _ = writeln!(std::io::stderr(), "cwlogs-retentie: could not write output");
    ExitCode::from(2)
}

fn run(cli: &Cli) -> Result<cwlogs_retentie::Output, Error> {
    let prices = if let Some(path) = &cli.prices {
        load_prices_file(path)?
    } else {
        cwlogs_retentie::prices_builtin()
    };
    let options = Options {
        prices,
        as_of: cli.as_of.clone(),
        fail_stored_bytes: cli.fail_stored_bytes,
        json: cli.json,
        markdown: cli.md,
        quiet: cli.quiet,
        verbose: cli.verbose,
    };
    execute_paths(&cli.files, &options)
}
