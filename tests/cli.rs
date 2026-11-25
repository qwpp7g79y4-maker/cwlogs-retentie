//! Command-line behavior: exit codes, warnings, and the file walker.

use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Held around every spawn of the binary that waits for it, so that a pipe end
/// of one test is never inherited by the child of another test in this file.
static SPAWN: Mutex<()> = Mutex::new(());

fn spawn_lock() -> MutexGuard<'static, ()> {
    SPAWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("cwlogs-retentie-cli-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn output_of(cmd: &mut Command) -> (i32, String, String) {
    let _guard = spawn_lock();
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cwlogs-retentie"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd.env("CWLOGS_RETENTIE_TODAY", "2026-11-20");
    cmd.args(args);
    cmd
}

fn run(args: &[&str]) -> (i32, String, String) {
    output_of(&mut command(args))
}

fn run_stdin(args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let _guard = spawn_lock();
    let mut cmd = command(args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn group(name: &str, stored: u64) -> String {
    format!(r#"{{"logGroupName":"{name}","storedBytes":{stored}}}"#)
}

#[test]
fn help_and_version_exit_0() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("no network"));
    assert!(stdout.contains(
        "Exit codes: 0 ok, 1 unretained group above --fail-stored-bytes, 2 input or usage error, 3 --quiet with --verbose or --json with --md."
    ));
    let (code, stdout, stderr) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("cwlogs-retentie 1.0.0"));
}

/// The commit of this tree as git reports it, or `None` when git cannot say.
fn tree_commit() -> Option<String> {
    let _guard = spawn_lock();
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let sha = text.trim();
    (output.status.success() && sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| sha.to_string())
}

#[test]
fn version_has_the_commit_in_debug_builds_only() {
    let commit = tree_commit();
    let expected = match (&commit, cfg!(debug_assertions)) {
        (Some(sha), true) => format!("cwlogs-retentie 1.0.0 ({sha})\n"),
        _ => "cwlogs-retentie 1.0.0\n".to_string(),
    };
    for flag in ["--version", "-V"] {
        let (code, stdout, stderr) = run(&[flag]);
        assert_eq!(code, 0, "{flag}: {stderr}");
        assert!(stderr.is_empty());
        assert_eq!(stdout, expected, "{flag}");
    }
}

#[test]
fn usage_errors_exit_2() {
    let (code, stdout, stderr) = run(&[]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("cwlogs-retentie"));
    let (code, _, stderr) = run(&["--no-such-flag", "x"]);
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
    let (code, _, stderr) = run(&["--fail-stored-bytes=-1", "x"]);
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
    let file = "examples/describe-log-groups.json";
    // Shell passes `--fail-stored-bytes=+5` as one argument.
    let (code, stdout, stderr) = run(&["--fail-stored-bytes=+5", file]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("`+5` is not a plain non-negative integer"),
        "{stderr}"
    );
    // Shell passes `--fail-stored-bytes +5` as a flag plus the argument `+5`.
    let (code, stdout, stderr) = run(&["--fail-stored-bytes", "+5", file]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("`+5` is not a plain non-negative integer"),
        "{stderr}"
    );
    let (code, stdout, stderr) = run(&["--fail-stored-bytes", "5", file]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("fail-stored-bytes: 5\n"), "{stdout}");
    assert!(stdout.contains("threshold exceeded: yes"), "{stdout}");
    let (code, _, stderr) = run(&["--as-of", "2025-02-29", "examples/describe-log-groups.json"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("invalid --as-of"));
    assert!(stderr.starts_with("cwlogs-retentie: "));
}

#[cfg(target_os = "linux")]
#[test]
fn unwritable_stdout_exits_2() {
    let mut cmd = command(&["examples/describe-log-groups.json"]);
    cmd.stdout(File::create("/dev/full").unwrap());
    let (code, _, stderr) = output_of_stderr(&mut cmd);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stderr, "cwlogs-retentie: could not write output\n");
}

#[cfg(target_os = "linux")]
fn output_of_stderr(cmd: &mut Command) -> (i32, (), String) {
    let _guard = spawn_lock();
    cmd.stderr(Stdio::piped());
    let out = cmd.spawn().unwrap().wait_with_output().unwrap();
    (
        out.status.code().unwrap(),
        (),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn as_of_accepts_only_digits() {
    for bad in ["2026-+1-09", "2026-01-+9", "+026-01-09"] {
        let (code, _, stderr) = run(&["--as-of", bad, "examples/describe-log-groups.json"]);
        assert_eq!(code, 2, "{bad}");
        assert!(stderr.contains("invalid --as-of"), "{bad}");
    }
    for good in ["2026-11-09", "2024-02-29"] {
        let (_, _, stderr) = run(&["--as-of", good, "examples/describe-log-groups.json"]);
        assert!(!stderr.contains("invalid --as-of"), "{good}");
    }
}

#[test]
fn stored_bytes_string_with_unicode_space_is_an_error() {
    for bad in ["\\u00a010", "\\u202810", "\\u300010", "10\\u00a0"] {
        let json = format!(r#"{{"logGroupName":"/a","storedBytes":"{bad}"}}"#);
        let (code, stdout, stderr) = run_stdin(&["-"], json.as_bytes());
        assert_eq!(code, 2, "{bad}");
        assert!(stdout.is_empty(), "{bad}: {stdout}");
        assert!(stderr.contains("bad storedBytes"), "{bad}: {stderr}");
    }
    let json = r#"{"logGroupName":"/a","storedBytes":" 10\t"}"#;
    let (code, _, _) = run_stdin(&["-"], json.as_bytes());
    assert_ne!(code, 2);
}

#[test]
fn missing_file_exits_2() {
    let (code, stdout, stderr) = run(&["missing-groups.json"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("missing-groups.json"));
    assert!(stderr.starts_with("cwlogs-retentie: "));
}

#[test]
fn stdin_dash_reads_one_group() {
    let (code, stdout, stderr) = run_stdin(&["-"], group("/from-stdin", 1).as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("/from-stdin"));
    assert!(stdout.contains("as-of: not set"));
}

#[cfg(unix)]
#[test]
fn stdin_read_error_is_reported_with_a_dash_prefix() {
    let scratch = Scratch::new();
    let mut cmd = command(&["-"]);
    cmd.stdin(Stdio::from(File::open(scratch.path()).unwrap()));
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("cwlogs-retentie: -: "), "{stderr}");
}

#[test]
fn exit_1_still_prints_the_report() {
    let scratch = Scratch::new();
    let path = scratch.path().join("groups.json");
    fs::write(&path, group("/open", 50)).unwrap();
    let (code, stdout, stderr) = run(&["--fail-stored-bytes=49", path.to_str().unwrap()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("threshold exceeded: yes"));
    assert!(stdout.contains("/open"));
    assert!(stdout.contains("Not executed"));
    let kept = scratch.path().join("kept.json");
    fs::write(
        &kept,
        r#"{"logGroupName":"/kept","storedBytes":50,"retentionInDays":7}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&["--fail-stored-bytes=0", kept.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("threshold exceeded: no"));
    assert!(!stdout.contains("put-retention-policy"));
}

#[test]
fn json_warnings_go_to_stderr_and_hide_the_token() {
    let scratch = Scratch::new();
    let path = scratch.path().join("page.json");
    fs::write(
        &path,
        r#"{"nextToken":"SECRETTOKEN99","logGroups":[{"logGroupName":"/a","storedBytes":1}]}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&["--json", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("warning:"));
    assert!(stderr.contains("nextToken was not followed"));
    assert!(!stderr.contains("SECRETTOKEN99"));
    assert!(stdout.contains("\"threshold_exceeded\": false"));
    assert!(stdout.ends_with('\n'));
    assert!(!stdout.contains("SECRETTOKEN99"));
}

#[test]
fn prices_override_uses_the_class_rate() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"eu-west-1","standard_storage_per_gb_month":"0.03","infrequent_access_storage_per_gb_month":"1.00","delivery_storage_per_gb_month":"0.03"}"#,
    )
    .unwrap();
    let groups = scratch.path().join("groups.json");
    fs::write(
        &groups,
        r#"{"logGroupName":"/a","storedBytes":1073741824,"logGroupClass":"INFREQUENT_ACCESS"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("source https://example.test/quote"));
    assert!(stdout.contains("date 2026-01-15"));
    assert!(stdout.contains("region eu-west-1"));
    assert!(stdout.contains("1.00"));
}

#[test]
fn invisible_fillers_in_price_source_are_escaped_and_cost_is_unchanged() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        "{\"currency\":\"USD\",\"source\":\"quote\u{3164}x\u{e0001}y\",\"date\":\"2026-01-15\",\"region\":\"eu-west-1\",\"standard_storage_per_gb_month\":\"0.03\",\"infrequent_access_storage_per_gb_month\":\"1.00\",\"delivery_storage_per_gb_month\":\"0.03\"}",
    )
    .unwrap();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1_073_741_824)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    for ch in ['\u{3164}', '\u{e0001}'] {
        assert!(!stdout.contains(ch), "{stdout}");
    }
    assert!(stdout.contains("\\u{3164}"), "{stdout}");
    assert!(stdout.contains("\\u{e0001}"), "{stdout}");
    assert!(stdout.contains("0.03"), "{stdout}");
    let (code, stdout, stderr) = run(&[
        "--json",
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    for ch in ['\u{3164}', '\u{e0001}'] {
        assert!(!stdout.contains(ch), "{stdout}");
    }
    assert!(
        stdout.contains("\"estimated_usd_month\": \"0.03\""),
        "{stdout}"
    );
}

#[test]
fn hangul_filler_in_price_source_is_escaped_and_cost_is_unchanged() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        "{\"currency\":\"USD\",\"source\":\"quote\u{115f}x\",\"date\":\"2026-01-15\",\"region\":\"eu-west-1\",\"standard_storage_per_gb_month\":\"0.03\",\"infrequent_access_storage_per_gb_month\":\"1.00\",\"delivery_storage_per_gb_month\":\"0.03\"}",
    )
    .unwrap();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1_073_741_824)).unwrap();
    for json in [false, true] {
        let mut args = vec![
            "--prices",
            prices.to_str().unwrap(),
            groups.to_str().unwrap(),
        ];
        if json {
            args.insert(0, "--json");
        }
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 0, "{stderr}");
        assert!(!stdout.contains('\u{115f}'), "{stdout}");
        assert!(stdout.contains("\\u{115f}"), "{stdout}");
        assert!(stdout.contains("0.03"), "{stdout}");
    }
}

#[test]
fn many_small_groups_total_is_not_the_sum_of_rounded_rows() {
    let scratch = Scratch::new();
    let path = scratch.path().join("many_small.json");
    let mut body = String::with_capacity(56_000);
    body.push_str("{\"logGroups\":[");
    for i in 0..1000 {
        if i > 0 {
            body.push(',');
        }
        body.push_str(&format!(
            r#"{{"logGroupName":"/g{i:04}","storedBytes":104857600}}"#
        ));
    }
    body.push_str("]}");
    assert!(body.len() < 80_000, "{}", body.len());
    fs::write(&path, &body).unwrap();

    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    let total = stdout
        .lines()
        .find(|line| line.starts_with("total"))
        .unwrap_or("");
    assert!(total.contains("104857600000"), "{total}");
    assert!(total.contains("97.65625"), "{total}");
    assert!(total.contains("2.93"), "{total}");
    assert!(!total.contains("0.00"), "{total}");
    let row = stdout
        .lines()
        .find(|line| line.contains("/g0000"))
        .unwrap_or("");
    assert!(row.contains("0.097656"), "{row}");
    assert!(row.contains("0.00"), "{row}");
    assert!(!row.contains("2.93"), "{row}");

    let (code, stdout, stderr) = run(&["--json", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    let (groups, totals) = stdout.split_once("\"totals\"").unwrap_or(("", ""));
    assert!(groups.contains("\"estimated_usd_month\": \"0.00\""));
    assert!(!groups.contains("\"estimated_usd_month\": \"2.93\""));
    assert!(
        totals.contains("\"stored_bytes\": 104857600000"),
        "{totals}"
    );
    assert!(totals.contains("\"gb\": \"97.65625\""), "{totals}");
    assert!(
        totals.contains("\"estimated_usd_month\": \"2.93\""),
        "{totals}"
    );
    assert!(
        !totals.contains("\"estimated_usd_month\": \"0.00\""),
        "{totals}"
    );
}

#[test]
fn prices_reject_a_directory() {
    let scratch = Scratch::new();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let dir = scratch.path().join("prices-dir");
    fs::create_dir(&dir).unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", dir.to_str().unwrap(), groups.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not a file"));
    assert!(stderr.starts_with("cwlogs-retentie: "));
}

#[cfg(unix)]
#[test]
fn prices_reject_a_fifo_without_reading_it() {
    let _guard = spawn_lock();
    let scratch = Scratch::new();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let fifo = scratch.path().join("prices.pipe");
    let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());
    // No external `timeout` command (macOS has none): the test kills a hung child itself.
    let mut cmd = command(&["--prices", fifo.to_str().unwrap(), groups.to_str().unwrap()]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let started = std::time::Instant::now();
    let mut hung = false;
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > std::time::Duration::from_secs(10) {
            hung = true;
            child.kill().unwrap();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    assert!(!hung, "the binary blocked on the FIFO");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(out.stdout.is_empty());
    assert!(stderr.contains("not a file"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn prices_follow_a_symlink_to_a_regular_file() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"eu-west-1","standard_storage_per_gb_month":"0.50","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}"#,
    )
    .unwrap();
    let link = scratch.path().join("prices-link.json");
    symlink(&prices, &link).unwrap();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1_073_741_824)).unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", link.to_str().unwrap(), groups.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("0.50"));
    assert!(stdout.contains("source https://example.test/quote"));
}

#[cfg(unix)]
#[test]
fn directory_symlink_is_skipped() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(walk.join("good.json"), group("/kept", 128)).unwrap();
    let outside = scratch.path().join("outside.json");
    fs::write(&outside, group("/other", 512)).unwrap();
    symlink(&outside, walk.join("link.json")).unwrap();
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("skipped symlink"));
    assert!(stdout.contains("/kept"));
    // The warning holds the scratch path, which may hold any digits; look at the table rows.
    assert!(!stdout.lines().any(|line| line.starts_with("/other")));
}

#[cfg(unix)]
#[test]
fn explicit_symlink_is_followed() {
    let scratch = Scratch::new();
    let real = scratch.path().join("real.json");
    fs::write(&real, group("/kept", 128)).unwrap();
    let link = scratch.path().join("link.json");
    symlink(&real, &link).unwrap();
    let (code, stdout, stderr) = run(&[link.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("/kept"));
    assert!(!stdout.contains("skipped symlink"));
}

#[test]
fn directory_at_64_levels_is_accepted() {
    let scratch = Scratch::new();
    let mut dir = scratch.path().to_path_buf();
    for i in 0..63 {
        dir.push(format!("n{i}"));
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.json"), group("/deep", 1)).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("/deep"));
}

#[test]
fn directory_deeper_than_64_levels_is_an_error() {
    let scratch = Scratch::new();
    let mut dir = scratch.path().to_path_buf();
    for i in 0..64 {
        dir.push(format!("n{i}"));
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.json"), group("/deep", 1)).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("directory is nested too deeply"));
    assert!(stderr.starts_with("cwlogs-retentie: "));
}

#[test]
fn file_larger_than_268435456_bytes() {
    let scratch = Scratch::new();
    let log = scratch.path().join("big.json");
    let file = File::create(&log).unwrap();
    file.set_len(268_435_457).unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("file is larger than 268435456 bytes"));
    assert!(!stderr.contains("SECRET"));
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", log.to_str().unwrap(), groups.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("file is larger than 268435456 bytes"));
    assert!(!stderr.contains("SECRET"));
}

#[test]
fn directory_skips_non_utf8_and_empty_members() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("empty.json"), b"").unwrap();
    let mut bad = vec![0xff, 0xfe];
    bad.extend(b"SECRETLEAK");
    fs::write(scratch.path().join("bad.bin"), bad).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("not UTF-8"));
    assert!(stdout.contains("no log groups"));
    assert!(stdout.contains("/kept"));
    assert!(!stdout.contains("SECRETLEAK"));
}

#[test]
fn truncated_json_in_a_directory_is_an_error() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(
        scratch.path().join("b.json"),
        r#"{"logGroups":[{"storedBytes":"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--fail-stored-bytes",
        "1000",
        scratch.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("b.json: not JSON"), "{stderr}");
    assert!(!stdout.contains("threshold exceeded: no"), "{stdout}");
}

#[test]
fn directory_skips_a_text_note_with_a_warning() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("notes.txt"), "hello notes").unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("/kept"));
    assert!(stdout.contains("notes.txt: not JSON"), "{stdout}");
    assert!(!stdout.contains("hello notes"));
}

#[test]
fn fail_stored_bytes_refuses_when_a_member_was_skipped() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("notes.txt"), "hello notes").unwrap();
    let dir = scratch.path().to_str().unwrap();
    let (code, stdout, stderr) = run(&["--fail-stored-bytes", "1000", dir]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("notes.txt"), "{stderr}");
    assert!(stderr.contains("cannot be judged"), "{stderr}");
    let (code, stdout, stderr) = run(&[dir]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("notes.txt: not JSON"), "{stdout}");
}

#[test]
fn json_without_log_groups_is_still_an_error() {
    let scratch = Scratch::new();
    let path = scratch.path().join("nothing.json");
    fs::write(&path, r#"{"nothing":1}"#).unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("expected log groups"), "{stderr}");
}

#[test]
fn corrupt_gzip_is_an_error() {
    let scratch = Scratch::new();
    let log = scratch.path().join("notes.txt");
    let mut bytes = vec![0x1f, 0x8b];
    bytes.extend(b"SECRETLEAK");
    fs::write(&log, bytes).unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("could not read gzip"));
    assert!(!stderr.contains("SECRETLEAK"));
}

#[cfg(unix)]
#[test]
fn directory_skips_a_fifo() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    let fifo = scratch.path().join("pipe");
    let status = {
        let _guard = spawn_lock();
        Command::new("mkfifo").arg(&fifo).status().unwrap()
    };
    assert!(status.success());
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("skipped non-regular file"));
    assert!(stdout.contains("/kept"));
}

#[test]
fn directory_walks_files_in_sorted_order() {
    let scratch = Scratch::new();
    fs::write(
        scratch.path().join("b.json"),
        r#"{"nextToken":"BBB","logGroups":[{"logGroupName":"/b","storedBytes":1}]}"#,
    )
    .unwrap();
    fs::write(
        scratch.path().join("a.json"),
        r#"{"nextToken":"AAA","logGroups":[{"logGroupName":"/a","storedBytes":1}]}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains("AAA"));
    assert!(!stdout.contains("BBB"));
    let a = stdout.find("a.json").unwrap();
    let b = stdout.find("b.json").unwrap();
    assert!(a < b, "{stdout}");
}

#[test]
fn bad_json_does_not_echo_the_body() {
    let scratch = Scratch::new();
    let path = scratch.path().join("bad.json");
    fs::write(&path, "NOTJSON SECRETFILEBODY").unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not JSON"));
    assert!(!stderr.contains("SECRETFILEBODY"));
    assert!(!stderr.contains("NOTJSON"));
}

#[cfg(unix)]
#[test]
fn control_chars_in_a_path_stay_on_one_line() {
    let scratch = Scratch::new();
    let missing = scratch.path().join("no\nFAKE: line.json");
    let (code, stdout, stderr) = run(&[missing.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(stderr.contains("\\u{a}"), "{stderr:?}");
    assert!(stderr.contains("FAKE: line.json"));
    assert!(!stderr.lines().any(|line| line.starts_with("FAKE")));
}

// Windows refuses file names with characters 1 to 31, so this test runs on Linux only.
#[cfg(target_os = "linux")]
#[test]
fn escape_and_bell_in_a_path_stay_on_one_line() {
    let scratch = Scratch::new();
    let bad = scratch.path().join("bad\u{1b}]0;pwn\u{7}.json");
    fs::write(&bad, b"{").unwrap();
    let (code, stdout, stderr) = run(&[bad.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(!stderr.as_bytes().contains(&0x1b));
    assert!(!stderr.as_bytes().contains(&0x07));
    assert!(stderr.contains("not JSON"));
}

#[test]
fn line_separator_and_bidi_override_are_not_raw() {
    let scratch = Scratch::new();
    let few = "{\n  \"logGroupName\": \"/plain\",\n  \"storedBytes\": 1\n}\n";
    let plain = scratch.path().join("plain.json");
    fs::write(&plain, few).unwrap();
    let (code, stdout, stderr) = run(&[plain.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("/plain"));
    assert!(!stdout.contains('\u{2028}'));
    assert!(!stdout.contains('\u{202e}'));

    let ordinary = scratch.path().join("ordinary.json");
    fs::write(&ordinary, "{\n  \"note\": \"no groups\"\n}\n").unwrap();
    let (code, stdout, stderr) = run(&[ordinary.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("ordinary.json"));
    // A Windows path has backslash separators of its own; the message adds none.
    let shown = ordinary.to_str().unwrap();
    assert!(!stderr.replace(shown, "").contains('\\'), "{stderr:?}");
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");

    let hidden = scratch.path().join("bad\u{2028}line\u{202e}name.json");
    fs::write(&hidden, "{\n  \"note\": \"no groups\"\n}\n").unwrap();
    let (code, stdout, stderr) = run(&[hidden.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(!stderr.contains('\u{2028}'));
    assert!(!stderr.contains('\u{202e}'));
    assert!(stderr.contains("\\u{2028}"));
    assert!(stderr.contains("\\u{202e}"));
    assert!(stderr.contains("expected log groups"));
}

#[cfg(unix)]
#[test]
fn symlink_warning_escapes_line_separator_and_bidi_override() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(
        walk.join("good.json"),
        "{\n  \"logGroupName\": \"/kept\",\n  \"storedBytes\": 1\n}\n",
    )
    .unwrap();
    let outside = scratch.path().join("out.json");
    fs::write(&outside, group("/other", 1)).unwrap();
    symlink(&outside, walk.join("link\u{2028}\u{202e}")).unwrap();
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains('\u{2028}'));
    assert!(!stderr.contains('\u{202e}'));
    assert!(!stdout.contains('\u{2028}'));
    assert!(!stdout.contains('\u{202e}'));
    assert!(stdout.contains("\\u{2028}"));
    assert!(stdout.contains("\\u{202e}"));
    assert!(stdout.contains("/kept"));
    assert!(!stdout.contains("/other"));
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.contains("skipped symlink"))
            .count(),
        1,
        "{stdout}"
    );
}

#[cfg(unix)]
#[test]
fn symlink_warning_escapes_the_path() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(walk.join("good.json"), group("/kept", 1)).unwrap();
    let outside = scratch.path().join("out.json");
    fs::write(&outside, group("/other", 1)).unwrap();
    symlink(&outside, walk.join("link\nFAKE")).unwrap();
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    let notes: Vec<_> = stdout
        .lines()
        .filter(|line| line.contains("skipped symlink"))
        .collect();
    assert_eq!(notes.len(), 1, "{stdout}");
    assert!(notes[0].contains("\\u{a}"), "{notes:?}");
    assert!(!stdout.lines().any(|line| line.starts_with("FAKE")));
    assert!(stdout.contains("/kept"));
    assert!(!stdout.contains("/other"));
}

#[test]
fn directory_total_is_the_sum_of_the_single_files() {
    let scratch = Scratch::new();
    let a = scratch.path().join("a.json");
    let b = scratch.path().join("b.json");
    fs::write(&a, group("/a", 1_000_000_000)).unwrap();
    fs::write(&b, group("/b", 2_000_000_000)).unwrap();
    let total = |args: &[&str]| {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 0, "{stderr}");
        stdout
    };
    let both = total(&["--json", scratch.path().to_str().unwrap()]);
    let one = total(&["--json", a.to_str().unwrap()]);
    let two = total(&["--json", b.to_str().unwrap()]);
    assert!(both.contains("/a") && both.contains("/b"), "{both}");
    assert!(one.contains("/a") && two.contains("/b"));
    let totals = |out: &str| {
        let block = out.split("\"totals\"").nth(1).expect("totals block");
        let field = |key: &str| {
            let rest = block.split(key).nth(1).expect("totals field");
            let value = rest.trim_start_matches(['"', ':', ' ']);
            value.split([',', '"', '\n']).next().unwrap().to_string()
        };
        (
            field("\"stored_bytes\"").parse::<u64>().unwrap(),
            field("\"estimated_usd_month\"").parse::<f64>().unwrap(),
        )
    };
    let (both_bytes, both_usd) = totals(&both);
    let (one_bytes, one_usd) = totals(&one);
    let (two_bytes, two_usd) = totals(&two);
    assert_eq!(both_bytes, one_bytes + two_bytes, "{both}");
    // The directory rounds once from the unrounded amount: 0.08.
    // The single files round on their own: 0.03 and 0.06, sum 0.09.
    let cents = |usd: f64| (usd * 100.0).round() as i64;
    assert_eq!(cents(both_usd), 8, "directory {both_usd}");
    assert_eq!(cents(one_usd), 3, "file a {one_usd}");
    assert_eq!(cents(two_usd), 6, "file b {two_usd}");
    assert_ne!(
        cents(both_usd),
        cents(one_usd) + cents(two_usd),
        "directory {both_usd} must not be the sum of rounded rows {one_usd} + {two_usd}"
    );
}

#[test]
fn rtl_isolate_and_mark_in_member_name_are_not_raw() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("tok\u{2067}.json"), "not json at all").unwrap();
    fs::write(scratch.path().join("mark\u{200f}.txt"), "hello").unwrap();
    let dir = scratch.path().to_str().unwrap();
    let (code, stdout, stderr) = run(&[dir]);
    assert_eq!(code, 0, "{stderr}");
    for ch in ['\u{2067}', '\u{200f}'] {
        assert!(!stdout.contains(ch), "{stdout:?}");
        assert!(!stderr.contains(ch), "{stderr:?}");
    }
    assert!(stdout.contains("tok\\u{2067}.json"), "{stdout:?}");
    assert!(stdout.contains("mark\\u{200f}.txt"), "{stdout:?}");
    let (code, stdout, stderr) = run(&["--json", dir]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains('\u{2067}') && !stderr.contains('\u{2067}'));
    assert!(stderr.contains("tok\\u{2067}.json"), "{stderr:?}");
    let (code, _, stderr) = run(&["--fail-stored-bytes", "1000", dir]);
    assert_eq!(code, 2, "{stderr}");
    assert!(!stderr.contains('\u{2067}'), "{stderr:?}");
}

#[test]
fn fractional_age_shows_at_most_six_decimals() {
    let scratch = Scratch::new();
    let file = scratch.path().join("age.json");
    // 2026-11-09 00:00 UTC is 1_794_182_400_000 ms. 100_000_001 ms before it gives a
    // non-terminating fraction of a day.
    let created = 1_794_182_400_000u64 - 100_000_001;
    fs::write(
        &file,
        format!(r#"{{"logGroupName":"/a","storedBytes":1073741824,"creationTime":{created}}}"#),
    )
    .unwrap();
    let path = file.to_str().unwrap();
    let digits = |text: &str| {
        let rest = text
            .split("\"age_days\": \"")
            .nth(1)
            .expect("age_days field");
        let value = rest.split('"').next().unwrap().to_string();
        value.split_once('.').map_or(0, |(_, frac)| frac.len())
    };
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", "--json", path]);
    assert_eq!(code, 0, "{stderr}");
    let n = digits(&stdout);
    assert!((1..=6).contains(&n), "{stdout}");
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", path]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("1.157407 "), "{stdout}");
    assert!(!stdout.contains("1.1574074"), "{stdout}");
}

#[test]
fn explicit_file_that_is_not_utf8_is_an_error() {
    let scratch = Scratch::new();
    let log = scratch.path().join("latin.json");
    fs::write(
        &log,
        b"{\"logGroupName\":\"/a\",\"storedBytes\":1}\xff\xfeSECRETLEAK",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("latin.json: not UTF-8"), "{stderr}");
    assert!(!stderr.contains("SECRETLEAK"));
}

#[test]
fn price_file_errors_carry_the_price_file_prefix() {
    let scratch = Scratch::new();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, "{\"currency\":\"USD\"}").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.starts_with("cwlogs-retentie: price file "),
        "{stderr}"
    );
    assert!(stderr.contains("prices.json: missing source"), "{stderr}");
}

#[test]
fn price_file_that_is_not_utf8_is_an_error() {
    let scratch = Scratch::new();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, b"\xff\xfeSECRETLEAK").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("prices.json: not UTF-8"), "{stderr}");
    assert!(!stderr.contains("SECRETLEAK"));
}

#[test]
fn stdin_larger_than_268435456_bytes_is_an_error() {
    let _guard = spawn_lock();
    let mut cmd = command(&["-"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let chunk = vec![b' '; 1 << 20];
        // The child may stop reading once it has seen the limit; a write error ends the loop.
        if (0..257).all(|_| stdin.write_all(&chunk).is_ok()) {
            let _ = stdin.write_all(b" ");
        }
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("-: file is larger than 268435456 bytes"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn unreadable_directory_is_an_error_with_its_path() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let dir = scratch.path().join("locked");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&dir).is_ok();
    let (code, stdout, stderr) = run(&[dir.to_str().unwrap()]);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        return; // running as a user who ignores permissions
    }
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("locked: "), "{stderr}");
    assert!(stderr.contains("ermission denied"), "{stderr}");
}

#[test]
fn clap_usage_errors_escape_raw_user_input() {
    let (code, stdout, stderr) = run(&["--bogus\u{1b}]0;x\u{7}\u{202e}\nFAKE: line"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    for ch in ['\u{1b}', '\u{7}', '\u{202e}'] {
        assert!(!stderr.contains(ch), "{stderr:?}");
    }
    assert!(
        !stderr.lines().any(|line| line.starts_with("FAKE")),
        "{stderr:?}"
    );
    assert!(stderr.contains("\\u{202e}"), "{stderr:?}");
    let (code, _, stderr) = run(&["--fail-stored-bytes", "1\u{2028}2", "x.json"]);
    assert_eq!(code, 2);
    assert!(!stderr.contains('\u{2028}'), "{stderr:?}");
    assert!(stderr.contains("\\u{2028}"), "{stderr:?}");
}

// macOS refuses file names that are not UTF-8, so this test runs on Linux only.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_file_name_is_shown_as_escaped_bytes() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let scratch = Scratch::new();
    let bad = scratch.path().join(OsStr::from_bytes(b"bad\xff.json"));
    fs::write(&bad, b"not a log group").unwrap();
    let mut cmd = command(&[]);
    cmd.arg(&bad);
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad\\xff.json"), "{stderr}");
    assert!(!stderr.contains('\u{FFFD}'));

    let good = scratch.path().join(OsStr::from_bytes(b"good\xff.json"));
    fs::write(&good, group("/ok", 1)).unwrap();
    let mut cmd = command(&[]);
    cmd.arg(&good);
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("/ok"));
    assert!(!stdout.contains('\u{FFFD}'));
}

#[cfg(target_os = "linux")]
mod stdout_closed {
    use super::{command, group, spawn_lock};
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::process::{Command, Stdio};

    const MESSAGE: &str = "cwlogs-retentie: could not write output\n";
    const EXAMPLE: &str = "examples/describe-log-groups.json";

    /// Runs the binary through `sh` with a redirection on standard output.
    fn sh(redirect: &str, arg: &str) -> (Option<i32>, String) {
        let _guard = spawn_lock();
        let bin = env!("CARGO_BIN_EXE_cwlogs-retentie");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(format!("exec \"$0\" {arg} {redirect}"))
            .arg(bin)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("CWLOGS_RETENTIE_TODAY", "2026-11-20")
            .stderr(Stdio::piped());
        let out = cmd.spawn().unwrap().wait_with_output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    #[test]
    fn closed_stdout_is_exit_2_for_a_report() {
        let (code, err) = sh(">&-", EXAMPLE);
        assert_eq!(code, Some(2), "{err}");
        assert_eq!(err, MESSAGE);
    }

    #[test]
    fn closed_stdout_still_shows_a_usage_or_input_error() {
        let (code, err) = sh(">&-", "--bogus");
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains("unexpected argument '--bogus'"), "{err}");
        let (code, err) = sh(">&-", "/nonexistent/x.json");
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains("/nonexistent/x.json: "), "{err}");
        assert!(!err.contains("could not write output"), "{err}");
    }

    #[test]
    fn closed_stdout_is_exit_2_for_help_and_version() {
        for arg in ["--help", "--version"] {
            let (code, err) = sh(">&-", arg);
            assert_eq!(code, Some(2), "{arg}: {err}");
            assert_eq!(err, MESSAGE, "{arg}");
            assert!(!err.contains("panicked"), "{err}");
        }
    }

    #[test]
    fn full_stdout_is_exit_2_for_report_help_and_version() {
        for arg in [EXAMPLE, "--help", "--version"] {
            let (code, err) = sh(">/dev/full", arg);
            assert_eq!(code, Some(2), "{arg}: {err}");
            assert_eq!(err, MESSAGE, "{arg}");
        }
    }

    #[test]
    fn full_stdout_via_a_file_handle_is_exit_2_for_help() {
        let _guard = spawn_lock();
        let mut cmd = command(&["--help"]);
        cmd.stdout(File::create("/dev/full").unwrap());
        cmd.stderr(Stdio::piped());
        let out = cmd.spawn().unwrap().wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(String::from_utf8_lossy(&out.stderr), MESSAGE);
    }

    #[test]
    fn dev_null_keeps_the_real_exit_code() {
        for redirect in [">/dev/null", "1<>/dev/null"] {
            for arg in [EXAMPLE, "--help", "--version"] {
                let (code, err) = sh(redirect, arg);
                assert_eq!(code, Some(0), "{redirect} {arg}: {err}");
                assert_eq!(err, "", "{redirect} {arg}");
            }
            assert_eq!(sh(redirect, "--bogus").0, Some(2), "{redirect}");
        }
    }

    #[test]
    fn null_stdio_handle_keeps_exit_0() {
        let _guard = spawn_lock();
        let mut cmd = command(&[EXAMPLE]);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());
        let out = cmd.spawn().unwrap().wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty());
        let null = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/null")
            .unwrap();
        let mut cmd = command(&[EXAMPLE]);
        cmd.stdout(null);
        cmd.stderr(Stdio::piped());
        let out = cmd.spawn().unwrap().wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0));
    }

    #[test]
    fn broken_pipe_is_exit_2_without_a_panic() {
        let _guard = spawn_lock();
        // The child blocks on standard input until the read end of its
        // standard output is gone, so the write always meets a broken pipe.
        let mut cmd = command(&["-"]);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        drop(child.stdout.take());
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(group("/aws/app", 10).as_bytes()).unwrap();
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{err}");
        assert_eq!(err, MESSAGE);
    }
}

#[test]
fn clap_errors_show_newline_cr_and_escape_as_unicode_escapes_on_one_line() {
    let hostile = "1\u{1b}[31m\nFAKE: line\r\u{202e}x";
    let unknown = "--x\u{1b}[31m\nFAKE: line\r\u{202e}x";
    let file = "examples/describe-log-groups.json";
    let cases: Vec<Vec<&str>> = vec![
        vec!["--fail-stored-bytes", hostile, file],
        vec![unknown, file],
        vec![unknown],
    ];
    for args in cases {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{args:?}: {stderr}");
        assert!(stdout.is_empty(), "{args:?}: {stdout}");
        for ch in ['\u{1b}', '\r', '\u{202e}'] {
            assert!(!stderr.contains(ch), "{args:?}: {stderr:?}");
        }
        assert!(
            !stderr.lines().any(|line| line.starts_with("FAKE")),
            "{args:?}: {stderr:?}"
        );
        for shown in ["\\u{1b}", "\\u{a}", "\\u{d}", "\\u{202e}"] {
            assert!(
                stderr.contains(shown),
                "{args:?}: missing {shown} in {stderr:?}"
            );
        }
    }
}

#[test]
fn rate_with_more_than_28_significant_digits_is_rejected_not_rounded() {
    let scratch = Scratch::new();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1_073_741_824)).unwrap();
    let price_file = |rate: &str| {
        let path = scratch.path().join(format!("rate-{}.json", rate.len()));
        let body = format!(
            r#"{{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"{rate}","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}}"#
        );
        fs::write(&path, body).unwrap();
        path
    };
    // 32 significant digits: the exact amount is ...00.00 (0.0049999999999 < 0.005),
    // but a rounded rate would print ...00.01.
    for rate in [
        "1000000000000000000.0049999999999",
        "1000000000000000000.0040000000000",
    ] {
        let prices = price_file(rate);
        let (code, stdout, stderr) = run(&[
            "--prices",
            prices.to_str().unwrap(),
            groups.to_str().unwrap(),
        ]);
        assert_eq!(code, 2, "{rate}: {stdout}");
        assert!(stdout.is_empty(), "{rate}: {stdout}");
        assert!(
            stderr.contains("bad standard_storage_per_gb_month"),
            "{stderr}"
        );
        assert!(!stderr.contains(rate), "{stderr}");
    }
    // 28 significant digits stay exact.
    let prices = price_file("12345678901234567890.12345678");
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("12345678901234567890.12"), "{stdout}");
}

#[test]
fn json_nested_127_levels_is_read_and_128_is_not_json() {
    let scratch = Scratch::new();
    let nested = |extra: usize| {
        // envelope, array and group are 3 levels; `x` adds `extra` more.
        let body = format!(
            r#"{{"logGroups":[{{"logGroupName":"/a","storedBytes":1,"x":{}{}}}]}}"#,
            "[".repeat(extra),
            "]".repeat(extra)
        );
        let path = scratch.path().join(format!("nested-{extra}.json"));
        fs::write(&path, body).unwrap();
        path
    };
    let ok = nested(124);
    let (code, stdout, stderr) = run(&[ok.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("/a"), "{stdout}");
    let deep = nested(125);
    let (code, stdout, stderr) = run(&[deep.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("nested-125.json: not JSON"), "{stderr}");
}

#[test]
fn directory_member_with_json_but_no_log_groups_stops_the_run() {
    // A text note is skipped with a warning; valid JSON without log groups is an error.
    let scratch = Scratch::new();
    fs::write(scratch.path().join("a.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("n.txt"), "42").unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 2, "{stdout}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("n.txt: expected log groups"), "{stderr}");
}

#[test]
fn fail_stored_bytes_refuses_when_a_non_utf8_member_was_skipped() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.json"), group("/kept", 1)).unwrap();
    fs::write(scratch.path().join("u16.json"), b"\xff\xfe{\x00}\x00").unwrap();
    let dir = scratch.path().to_str().unwrap();
    let (code, stdout, stderr) = run(&["--fail-stored-bytes", "1000", dir]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("u16.json: not UTF-8"), "{stderr}");
    assert!(stderr.contains("cannot be judged"), "{stderr}");
}

#[test]
fn json_output_shows_the_price_source_as_escaped_text() {
    // In `--json` the source is the escaped description, not the original characters.
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"x\u202ey","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"0.03","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}"#,
    )
    .unwrap();
    let groups = scratch.path().join("groups.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--json",
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains(r#""source": "x\\u{202e}y""#), "{stdout}");
    assert!(!stdout.contains('\u{202e}'));
}

// Non-UTF-8 arguments and names exist on Linux only (macOS refuses such names).
#[cfg(target_os = "linux")]
#[test]
fn clap_error_with_a_non_utf8_argument_stays_on_one_line() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    for raw in [
        &b"--\xff\nFAKE: line"[..],
        &b"--\xff\x1b[31mRED\nFAKE: line"[..],
        &b"-\xff\r\xe2\x80\xaeFAKE: line"[..],
    ] {
        let mut cmd = command(&[]);
        cmd.arg(OsStr::from_bytes(raw));
        let (code, stdout, stderr) = output_of_lossy(&mut cmd);
        assert_eq!(code, 2, "{stderr:?}");
        assert!(stdout.is_empty(), "{stdout:?}");
        for ch in ['\u{1b}', '\r', '\u{202e}'] {
            assert!(!stderr.contains(ch), "{stderr:?}");
        }
        assert!(
            !stderr.lines().any(|line| line.starts_with("FAKE")),
            "{stderr:?}"
        );
        // A short flag is reported by its first character only.
        if raw.starts_with(b"--") {
            assert!(stderr.contains("\\xff"), "{stderr:?}");
        }
    }
}

#[cfg(target_os = "linux")]
fn output_of_lossy(cmd: &mut Command) -> (i32, String, String) {
    let _guard = spawn_lock();
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[cfg(target_os = "linux")]
#[test]
fn newline_in_a_non_utf8_path_is_shown_as_a_unicode_escape() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let mut cmd = command(&[]);
    cmd.arg(OsStr::from_bytes(b"no\xff\nsuch\r\x1b[31m"));
    let (code, _, stderr) = output_of_lossy(&mut cmd);
    assert_eq!(code, 2, "{stderr:?}");
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(
        stderr.starts_with("cwlogs-retentie: no\\xff\\u{a}such\\u{d}\\u{1b}[31m: "),
        "{stderr:?}"
    );
}

#[test]
fn a_file_name_that_starts_with_skipped_is_not_a_skipped_member() {
    let scratch = Scratch::new();
    let page = scratch.path().join("skipped a.json");
    fs::write(
        &page,
        r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1,"retentionInDays":7}],"nextToken":"abc"}"#,
    )
    .unwrap();
    // The label is the path as given, so run next to the file to get a name that starts with "skipped".
    let mut cmd = command(&["--fail-stored-bytes", "100", "skipped a.json"]);
    cmd.current_dir(scratch.path());
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("may be partial"), "{stdout}");
    // A really skipped member still stops the judgment.
    let dir = scratch.path().join("dir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("a.json"), group("/a", 1)).unwrap();
    fs::write(dir.join("notes.txt"), "hello").unwrap();
    let (code, _, stderr) = run(&["--fail-stored-bytes", "100", dir.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("skipped "), "{stderr}");
}

#[test]
fn a_half_cent_from_a_30_digit_gb_rounds_up_like_the_exact_amount() {
    // 1 byte is 2^-30 GB, which has 30 decimals. At 5368709.12 USD/GB the exact
    // amount is 0.005 USD, so half away from zero gives 0.01.
    let scratch = Scratch::new();
    let groups = scratch.path().join("one.json");
    fs::write(&groups, group("/a", 1)).unwrap();
    let prices = scratch.path().join("rate.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"5368709.12","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        "--json",
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains(r#""estimated_usd_month": "0.01""#),
        "{stdout}"
    );
}

#[test]
fn retention_as_a_string_of_digits_is_read_like_the_number() {
    let scratch = Scratch::new();
    let rows = |value: &str| {
        let path = scratch.path().join("r.json");
        fs::write(
            &path,
            format!(r#"{{"logGroups":[{{"logGroupName":"/a","storedBytes":1,"retentionInDays":{value}}}]}}"#),
        )
        .unwrap();
        run(&[path.to_str().unwrap()])
    };
    for good in [r#""14""#, r#""0014""#, "14"] {
        let (code, stdout, stderr) = rows(good);
        assert_eq!(code, 0, "{good}: {stderr}");
        assert!(
            stdout
                .lines()
                .any(|l| l.starts_with("/a ") && l.contains(" 14 ")),
            "{good}: {stdout}"
        );
    }
    for bad in [r#""+14""#, r#""1.0""#, "14.0"] {
        let (code, _, stderr) = rows(bad);
        assert_eq!(code, 2, "{bad}");
        assert!(stderr.contains("bad retentionInDays"), "{bad}: {stderr}");
    }
}

#[test]
fn total_of_stored_bytes_over_u64_is_an_input_error_without_a_name() {
    let scratch = Scratch::new();
    let path = scratch.path().join("two.json");
    fs::write(
        &path,
        r#"{"logGroups":[{"logGroupName":"/a","storedBytes":18446744073709551615},{"logGroupName":"/b","storedBytes":18446744073709551615}]}"#,
    )
    .unwrap();
    for json in [false, true] {
        let mut args = vec![path.to_str().unwrap()];
        if json {
            args.insert(0, "--json");
        }
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{stdout}");
        assert_eq!(stderr, "cwlogs-retentie: stored bytes overflow\n");
    }
    // Each group alone is the largest accepted value.
    fs::write(&path, group("/a", u64::MAX)).unwrap();
    assert_eq!(run(&[path.to_str().unwrap()]).0, 0);
}

#[test]
fn a_digit_string_is_at_most_20_characters_also_with_leading_zeros() {
    let scratch = Scratch::new();
    let path = scratch.path().join("s.json");
    let run_with = |stored: &str, retention: &str| {
        fs::write(
            &path,
            format!(
                r#"{{"logGroupName":"/a","storedBytes":{stored},"retentionInDays":{retention}}}"#
            ),
        )
        .unwrap();
        run(&[path.to_str().unwrap()])
    };
    let (code, _, stderr) = run_with(r#""00000000000000000014""#, r#""0014""#);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = run_with(r#""018446744073709551615""#, "14");
    assert_eq!(code, 2);
    assert!(stderr.contains("bad storedBytes"), "{stderr}");
    let (code, _, stderr) = run_with("1", r#""000000000000000000014""#);
    assert_eq!(code, 2);
    assert!(stderr.contains("bad retentionInDays"), "{stderr}");
}

#[test]
fn each_differing_arn_region_is_one_warning() {
    let scratch = Scratch::new();
    let path = scratch.path().join("regions.json");
    let arn = |region: &str, name: &str| {
        format!(
            r#"{{"logGroupName":"{name}","storedBytes":1,"arn":"arn:aws:logs:{region}:123456789012:log-group:{name}"}}"#
        )
    };
    let body = |items: &[String]| format!(r#"{{"logGroups":[{}]}}"#, items.join(","));
    let warnings = |items: &[String]| {
        fs::write(&path, body(items)).unwrap();
        let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
        assert_eq!(code, 0, "{stderr}");
        assert!(!stdout.contains("123456789012"));
        stdout
            .lines()
            .filter(|l| l.starts_with("warning: log group region "))
            .count()
    };
    let same = [arn("eu-west-1", "/a"), arn("eu-west-1", "/b")];
    assert_eq!(warnings(&same), 1);
    let two = [
        arn("eu-west-1", "/a"),
        arn("eu-west-2", "/b"),
        arn("eu-west-2", "/c"),
    ];
    assert_eq!(warnings(&two), 2);
}

/// Amounts are exact and rounded once; one that does not fit is an error. The README
/// ("Money precision") states the cases, and this test pins them, so a change of the
/// formula has to change the README too.
#[test]
fn money_precision_limits_are_as_documented() {
    let scratch = Scratch::new();
    let groups = scratch.path().join("g.json");
    let prices = scratch.path().join("p.json");
    // 60 days before 2026-11-09, in milliseconds since the epoch.
    let created_ms: u64 = 1_788_998_400_000;
    let measure = |rate: &str, bytes: u64| {
        fs::write(
            &groups,
            format!(r#"{{"logGroupName":"/a","storedBytes":{bytes},"creationTime":{created_ms}}}"#),
        )
        .unwrap();
        fs::write(
            &prices,
            format!(
                r#"{{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"{rate}","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}}"#
            ),
        )
        .unwrap();
        let (code, stdout, stderr) = run(&[
            "--json",
            "--as-of",
            "2026-11-09",
            "--prices",
            prices.to_str().unwrap(),
            groups.to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "{stderr}");
        let field = |name: &str| {
            let key = format!(r#""{name}": ""#);
            let start = stdout.find(&key).unwrap() + key.len();
            stdout[start..start + stdout[start..].find('"').unwrap()].to_string()
        };
        (field("estimated_usd_month"), field("save_30_usd"))
    };
    // 2 bytes, 60 days old: 1 byte is saved; exact 0.005 per byte is 0.01 in both columns.
    assert_eq!(measure("5368709.12", 2), ("0.01".into(), "0.01".into()));
    // Just under half a cent exactly, shown as 0.00 now that the amount is exact.
    assert_eq!(measure("5368709.1199999999999999999999", 1).0, "0.00");
    assert_eq!(measure("1789569.7066666666666666666666", 3).0, "0.00");
    // 1000000000000000000 per GB and u64::MAX bytes: ...425.38 in cents does not fit the
    // decimal type, so it is an error and not a cut amount.
    fs::write(
        &groups,
        format!(r#"{{"logGroupName":"/a","storedBytes":{}}}"#, u64::MAX),
    )
    .unwrap();
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"1000000000000000000","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert!(stderr.contains("price overflow"), "{stderr}");
}

/// Cents of `bytes` stored at 0.03 USD per GB, half away from zero, in integers.
fn oracle_cents(bytes: u128, mul: u128, div: u128) -> u128 {
    // cents = bytes * 3 * mul / (div * 2^30), rounded half up.
    let numer = bytes * 3 * mul;
    let denom = div * (1 << 30);
    (2 * numer + denom) / (2 * denom)
}

fn cents_text(cents: u128) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

fn json_of(args: &[&str], input: &str) -> serde_json::Value {
    let (code, stdout, stderr) = run_stdin(args, input.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    serde_json::from_str(&stdout).unwrap()
}

/// 2026-11-09 at midnight UTC, in milliseconds.
const AS_OF_MS: u128 = 1_794_182_400_000;

#[test]
fn a_total_that_is_an_exact_half_cent_rounds_up() {
    let input = r#"[{"logGroupName":"/a","storedBytes":161730679452},{"logGroupName":"/b","storedBytes":561861173778},{"logGroupName":"/c","storedBytes":162782022482}]"#;
    let json = json_of(&["--json", "-"], input);
    assert_eq!(json["totals"]["estimated_usd_month"], "24.77");
    let (code, stdout, _) = run_stdin(&["-"], input.as_bytes());
    assert_eq!(code, 0);
    let total = stdout.lines().find(|l| l.starts_with("total")).unwrap();
    assert!(total.ends_with("24.77"), "{total}");
}

#[test]
fn totals_of_odd_multiples_of_2_pow_29_match_the_integer_oracle() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..40 {
        // An odd multiple of 2^29 bytes in total makes the exact amount a half cent, split
        // at random so that every row has 29 or more digits.
        let groups = 3 + round % 3;
        let total: u128 = (u128::from(next() % 4000) * 2 + 1) << 29;
        let mut left = total;
        let mut parts = Vec::new();
        for i in 0..groups - 1 {
            let take = u128::from(next()) % (left / 2).max(1) + 1;
            left -= take;
            parts.push(group(&format!("/g{i}"), take as u64));
        }
        parts.push(group("/last", left as u64));
        let json = json_of(&["--json", "-"], &format!("[{}]", parts.join(",")));
        assert_eq!(
            json["totals"]["estimated_usd_month"],
            cents_text(oracle_cents(total, 1, 1)),
            "total {total}"
        );
    }
}

#[test]
fn a_saving_that_is_an_exact_half_cent_rounds_up() {
    let input = r#"{"logGroupName":"/a","storedBytes":492042190848,"creationTime":1791475200000}"#;
    let json = json_of(&["--json", "--as-of", "2026-11-09", "-"], input);
    assert_eq!(json["log_groups"][0]["save_30_usd"], "0.59");
    assert_eq!(json["totals"]["save_30_usd"], "0.59");
    assert!(json["log_groups"][0]["advice"][0]
        .as_str()
        .unwrap()
        .contains("30 days saves 0.59 USD/month"));
    let (code, stdout, _) = run_stdin(&["--as-of", "2026-11-09", "-"], input.as_bytes());
    assert_eq!(code, 0);
    assert!(stdout.contains("30 days saves 0.59 USD/month"), "{stdout}");
}

#[test]
fn savings_match_the_integer_oracle_for_ages_retentions_and_horizons() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut cases = 0;
    for retention in [None, Some(60_u128), Some(90), Some(180)] {
        for _ in 0..60 {
            let stored = u128::from(next() % (1 << 41)) + 1;
            let age_ms = u128::from(next() % (400 * 86_400_000)) + 1;
            let created = AS_OF_MS - age_ms;
            let ret = retention.map_or(String::new(), |r| format!(r#","retentionInDays":{r}"#));
            let input = format!(
                r#"{{"logGroupName":"/a","storedBytes":{stored},"creationTime":{created}{ret}}}"#
            );
            let json = json_of(&["--json", "--as-of", "2026-11-09", "-"], &input);
            // horizon = hn / hd days: the age, or the retention when it is shorter.
            let (hn, hd) = match retention {
                Some(r) if r * 86_400_000 <= age_ms => (r, 1),
                _ => (age_ms, 86_400_000),
            };
            for (target, key) in [
                (30_u128, "save_30_usd"),
                (90, "save_90_usd"),
                (365, "save_365_usd"),
            ] {
                let want = if target * hd >= hn {
                    "0.00".to_string()
                } else {
                    // saved = stored * (hn - target*hd) / hn
                    cents_text(oracle_cents(stored * (hn - target * hd), 1, hn))
                };
                assert_eq!(
                    json["log_groups"][0][key], want,
                    "stored {stored} age_ms {age_ms} retention {retention:?} {key}"
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 720);
}

#[test]
fn a_savings_total_is_rounded_once_from_the_exact_rows() {
    // Three groups of the same age: the total saving is the saving of the summed bytes.
    let created = 1_791_475_200_000_u128;
    let parts = [161_730_679_452_u128, 561_861_173_778, 162_782_022_482];
    let rows: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(r#"{{"logGroupName":"/g{i}","storedBytes":{b},"creationTime":{created}}}"#)
        })
        .collect();
    let json = json_of(
        &["--json", "--as-of", "2026-11-09", "-"],
        &format!("[{}]", rows.join(",")),
    );
    let age_ms = AS_OF_MS - created;
    let sum: u128 = parts.iter().sum();
    let want = cents_text(oracle_cents(sum * (age_ms - 30 * 86_400_000), 1, age_ms));
    assert_eq!(json["totals"]["save_30_usd"], want);
    // The group is younger than 90 days, so nothing more is removed.
    assert_eq!(json["totals"]["save_90_usd"], "0.00");
    assert_eq!(json["totals"]["estimated_usd_month"], "24.77");
}

#[test]
fn every_class_rate_is_exact_in_rows_and_totals() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("p.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test/quote","date":"2026-01-15","region":"us-east-1","standard_storage_per_gb_month":"0.03","infrequent_access_storage_per_gb_month":"0.015","delivery_storage_per_gb_month":"0.0075"}"#,
    )
    .unwrap();
    let input = r#"[{"logGroupName":"/a","storedBytes":161730679452},{"logGroupName":"/b","storedBytes":561861173778,"logGroupClass":"INFREQUENT_ACCESS"},{"logGroupName":"/c","storedBytes":162782022482,"logGroupClass":"DELIVERY"},{"logGroupName":"/d","storedBytes":1073741824,"logGroupClass":"DELIVERY"}]"#;
    let json = json_of(
        &["--json", "--prices", prices.to_str().unwrap(), "-"],
        input,
    );
    // 161730679452*0.03 + 561861173778*0.015 + (162782022482+1073741824)*0.0075, over 2^30.
    let numer: u128 = 161_730_679_452 * 3000 + 561_861_173_778 * 1500 + 163_855_764_306 * 750;
    // cents = numer / (10^5 * 2^30) * 100 rounded half up.
    let denom: u128 = 100_000 * (1 << 30) / 100;
    let cents = (2 * numer + denom) / (2 * denom);
    assert_eq!(json["totals"]["estimated_usd_month"], cents_text(cents));
}

#[test]
fn savings_at_an_exact_half_cent_with_a_retention_round_up() {
    // (stored bytes, age in ms, retention days, 30-day saving); each exact saving ends in
    // half a cent. Found by an exact-fraction search.
    let cases: [(u64, u64, u32, &str); 6] = [
        (21_562_614_874_112, 13_218_009_383, 90, "401.64"),
        (332_402_819_858_432, 9_989_469_777, 90, "6191.49"),
        (995_585_230_372_864, 23_820_211_200, 180, "23180.28"),
        (1_934_115_041_443_840, 32_548_324_423, 180, "45032.13"),
        (135_431_324_696_576, 20_509_352_574, 90, "2522.61"),
        (10_257_455_644_672, 16_653_496_471, 180, "238.83"),
    ];
    for (stored, age_ms, retention, want) in cases {
        let created = AS_OF_MS as u64 - age_ms;
        let input = format!(
            r#"{{"logGroupName":"/a","storedBytes":{stored},"creationTime":{created},"retentionInDays":{retention}}}"#
        );
        let json = json_of(&["--json", "--as-of", "2026-11-09", "-"], &input);
        assert_eq!(
            json["log_groups"][0]["save_30_usd"], want,
            "{stored} {age_ms}"
        );
        assert_eq!(json["totals"]["save_30_usd"], want, "{stored} {age_ms}");
    }
}

/// Groups whose savings at `day_span - 1` days sum to an exact half cent plus a
/// whole number of cents, with a different age on every group.
///
/// Two groups of age `day_span` days hold `day_span * 2^29` bytes between them, so at
/// rate 0.03 the saving is exactly 0.015. Each later group is
/// `256 * age` bytes at `target + 2^22 * e` milliseconds and adds exactly `0.03 * e`
/// dollars. The total of e = 1..=1145 is 19682.57.
fn half_cent_ages(day_span: u64) -> String {
    let target = day_span - 1;
    let mut parts = Vec::with_capacity(1147);
    let anchor_age = day_span * 86_400_000;
    let anchor_bytes = day_span * (1_u64 << 29);
    let anchor_created = AS_OF_MS as u64 - anchor_age;
    parts.push(format!(
        r#"{{"logGroupName":"/a","storedBytes":1,"creationTime":{anchor_created}}}"#
    ));
    parts.push(format!(
        r#"{{"logGroupName":"/b","storedBytes":{},"creationTime":{anchor_created}}}"#,
        anchor_bytes - 1
    ));
    for e in 1..=1145_u64 {
        let age = target * 86_400_000 + (1_u64 << 22) * e;
        let created = AS_OF_MS as u64 - age;
        parts.push(format!(
            r#"{{"logGroupName":"/e{e}","storedBytes":{},"creationTime":{created}}}"#,
            256 * age
        ));
    }
    format!("[{}]", parts.join(","))
}

#[test]
fn distinct_ages_on_an_exact_half_cent_round_once_per_horizon() {
    // Whole-number arithmetic (fraction of bytes, rate 3/100, and age in milliseconds)
    // gives these totals. Each saving total is exactly a half cent plus whole cents.
    let cases = [
        (31_u64, "save_30_usd", "40910.66", "19682.57"),
        (91, "save_90_usd", "83366.85", "19682.57"),
        (366, "save_365_usd", "277957.73", "19682.57"),
    ];
    for (day_span, key, usd, save) in cases {
        let input = half_cent_ages(day_span);
        let (code, stdout, stderr) =
            run_stdin(&["--json", "--as-of", "2026-11-09", "-"], input.as_bytes());
        assert_eq!(code, 0, "span {day_span}: {stderr}");
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            json["totals"]["estimated_usd_month"], usd,
            "span {day_span}"
        );
        assert_eq!(json["totals"][key], save, "span {day_span}");
    }
}

/// `pairs` pairs of groups. Pair k has its own age, 30 days plus `u_k` milliseconds, and the
/// two groups of a pair hold `age_ms` bytes between them, so the pair saves exactly
/// `u_k` bytes at 30 days. The names sort every `/a` before every `/b`, so the groups of a
/// pair are far apart. The `u_k` are 1, 2, ... and one large last value that makes their sum
/// `3 * 2^29`, so the saving at 30 days is exactly 0.045 and the total rounds up to 0.05.
fn interleaved_pairs(pairs: u64) -> String {
    let target_ms = 30 * 86_400_000_u64;
    let mut ages: Vec<u64> = (1..pairs).map(|u| target_ms + u).collect();
    let rest = 3 * (1_u64 << 29) - (1..pairs).sum::<u64>();
    ages.push(target_ms + rest);
    let mut parts = Vec::with_capacity(2 * ages.len());
    for (side, name) in [("a", 0_u64), ("b", 1)] {
        for (k, age) in ages.iter().enumerate() {
            let first = age / 2;
            let bytes = if name == 0 { first } else { age - first };
            let created = AS_OF_MS as u64 - age;
            parts.push(format!(
                r#"{{"logGroupName":"/{side}{k:06}","storedBytes":{bytes},"creationTime":{created}}}"#
            ));
        }
    }
    format!("[{}]", parts.join(","))
}

/// Runs the binary on a file and kills it after `seconds`, so a hang is a failed test.
fn run_file_limited(args: &[&str], contents: &str, seconds: u64) -> Option<(i32, String, String)> {
    let _guard = spawn_lock();
    let scratch = Scratch::new();
    let input = scratch.path().join("groups.json");
    fs::write(&input, contents).unwrap();
    let out_path = scratch.path().join("out.txt");
    let err_path = scratch.path().join("err.txt");
    let mut cmd = command(args);
    cmd.arg(&input);
    cmd.stdin(Stdio::null());
    cmd.stdout(File::create(&out_path).unwrap());
    cmd.stderr(File::create(&err_path).unwrap());
    let mut child = cmd.spawn().unwrap();
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(seconds) {
            child.kill().unwrap();
            child.wait().unwrap();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    Some((
        status.code().unwrap(),
        fs::read_to_string(out_path).unwrap(),
        fs::read_to_string(err_path).unwrap(),
    ))
}

#[test]
fn interleaved_pairs_on_an_exact_half_cent_do_not_hang() {
    // Whole-number arithmetic with Python fractions: 1000 pairs give a monthly total of
    // 72.46 and a 30-day saving of exactly 0.045, which is 0.05. On the base commit the
    // exact fallback needed over a minute for this file.
    let input = interleaved_pairs(1000);
    let (code, stdout, stderr) = run_file_limited(&["--json", "--as-of", "2026-11-09"], &input, 20)
        .expect("the run took longer than 20 seconds");
    assert_eq!(code, 0, "{stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["totals"]["estimated_usd_month"], "72.46");
    assert_eq!(json["totals"]["save_30_usd"], "0.05");
    assert_eq!(json["totals"]["save_90_usd"], "0.00");
}

const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

fn estimate_groups(scratch: &Scratch) -> PathBuf {
    let path = scratch.path().join("estimate.json");
    fs::write(
        &path,
        r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1073741824}]}"#,
    )
    .unwrap();
    path
}

#[test]
fn text_output_labels_the_monthly_cost_as_an_estimate() {
    let scratch = Scratch::new();
    let path = estimate_groups(&scratch);
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout.matches(ESTIMATE_LINE).count(), 1, "{stdout}");
    let header = stdout
        .lines()
        .find(|line| line.starts_with("log_group"))
        .expect("table header");
    assert!(header.contains("estimated_usd_month"), "{header}");
    assert!(
        !header.split_whitespace().any(|word| word == "usd_month"),
        "{header}"
    );
}

#[test]
fn json_output_labels_the_monthly_cost_as_an_estimate() {
    let scratch = Scratch::new();
    let path = estimate_groups(&scratch);
    let (code, stdout, stderr) = run(&["--json", "--as-of", "2026-11-09", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\n  \"estimate\": true,"), "{stdout}");
    assert!(
        stdout.contains(&format!("\"estimate_note\": \"{ESTIMATE_LINE}\"")),
        "{stdout}"
    );
    // Every place with a monthly cost uses the one name: the row and the total.
    assert_eq!(
        stdout.matches("\"estimated_usd_month\": \"0.03\"").count(),
        2,
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"usd_month\"") && !stdout.contains("\"usd_month\":"),
        "{stdout}"
    );
}

const STALE_WARNING_PREFIX: &str = "warning: price data dated ";

fn stale_prices(scratch: &Scratch, date: &str) -> PathBuf {
    let path = scratch.path().join("old-prices.json");
    fs::write(
        &path,
        format!(
            r#"{{"currency":"USD","source":"https://example.test/quote","date":"{date}","region":"us-east-1","standard_storage_per_gb_month":"0.03","infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}}"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn builtin_prices_older_than_180_days_at_as_of_warn_on_stderr() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    // The built-in price date is 2026-11-09; 180 days later is 2027-05-08.
    let (code, stdout, stderr) = run(&["--as-of", "2027-05-09", groups.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.starts_with(STALE_WARNING_PREFIX), "{stderr}");
    assert!(stderr.contains("2026-11-09"), "{stderr}");
    assert!(stderr.contains("181 days"), "{stderr}");
    assert!(stderr.contains("2027-05-09"), "{stderr}");
    assert!(!stdout.contains(STALE_WARNING_PREFIX), "{stdout}");
    let (code, _, stderr) = run(&["--as-of", "2027-05-08", groups.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "", "exactly 180 days is not older than 180 days");
}

#[test]
fn prices_file_older_than_180_days_warns_with_json_and_keeps_exit_0() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    let prices = stale_prices(&scratch, "2026-01-01");
    let (code, stdout, stderr) = run(&[
        "--json",
        "--prices",
        prices.to_str().unwrap(),
        "--as-of",
        "2026-12-01",
        groups.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with(STALE_WARNING_PREFIX), "{stderr}");
    assert!(stderr.contains("2026-01-01"), "{stderr}");
    assert!(stdout.contains("\"warnings\": []"), "{stdout}");
}

#[test]
fn prices_file_within_180_days_does_not_warn_and_a_future_date_does_not_warn() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    let recent = stale_prices(&scratch, "2026-06-01");
    let (code, _, stderr) = run(&[
        "--prices",
        recent.to_str().unwrap(),
        "--as-of",
        "2026-11-09",
        groups.to_str().unwrap(),
    ]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    let (code, _, stderr) = run(&[
        "--prices",
        recent.to_str().unwrap(),
        "--as-of",
        "2025-01-01",
        groups.to_str().unwrap(),
    ]);
    assert_eq!((code, stderr.as_str()), (0, ""));
}

#[test]
fn without_as_of_the_price_age_is_measured_against_today() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    let prices = stale_prices(&scratch, "2000-01-01");
    // The pinned day decides, and the real clock is the fallback without it.
    let args = [
        "--prices",
        prices.to_str().unwrap(),
        groups.to_str().unwrap(),
    ];
    let (code, _, stderr) = run(&args);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.starts_with(STALE_WARNING_PREFIX), "{stderr}");
    assert!(
        stderr.contains("2000-01-01") && stderr.contains("2026-11-20"),
        "{stderr}"
    );
    let mut pinned_early = command(&args);
    pinned_early.env("CWLOGS_RETENTIE_TODAY", "2000-06-01");
    let (code, _, stderr) = output_of(&mut pinned_early);
    assert_eq!((code, stderr.as_str()), (0, ""));
    let mut clock = command(&args);
    clock.env_remove("CWLOGS_RETENTIE_TODAY");
    let (code, _, stderr) = output_of(&mut clock);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.starts_with(STALE_WARNING_PREFIX), "{stderr}");
    assert!(stderr.contains("2000-01-01"), "{stderr}");
}

// The shape of a `describe-log-groups` response is documented at
// https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_DescribeLogGroups.html
// and the fields of one entry at
// https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_LogGroup.html
// (`logGroups` and an optional `nextToken`; each entry may hold `arn`, `creationTime`,
// `dataProtectionStatus`, `inheritedProperties`, `kmsKeyId`, `logGroupArn`, `logGroupClass`,
// `logGroupName`, `metricFilterCount`, `retentionInDays`, and `storedBytes`). This file is
// written from that documentation; nothing was fetched.
const DOCUMENTED: &str = "tests/fixtures/describe-log-groups-documented.json";

fn json_field<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let needle = format!("\"{key}\": \"");
    text.match_indices(&needle)
        .map(|(at, _)| {
            let start = at + needle.len();
            &text[start..start + text[start..].find('"').unwrap()]
        })
        .collect()
}

#[test]
fn documented_shape_with_arn_and_log_group_arn_prices_exactly() {
    let (code, stdout, stderr) = run(&["--json", "--as-of", "2026-11-09", DOCUMENTED]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    // Rows are sorted by name: delivery, ecs, checkout, worker, rds, then the total.
    // 0 + 20 + 100 + 5 + 10 GiB at 0.03 is 0.00 + 0.60 + 3.00 + 0.15 + 0.30 = 4.05.
    assert_eq!(
        json_field(&stdout, "estimated_usd_month"),
        ["0.00", "0.60", "3.00", "0.15", "0.30", "4.05"]
    );
    assert!(
        stdout.contains("\"stored_bytes\": 144955146240"),
        "{stdout}"
    );
    assert!(stdout.contains("\"gb\": \"135\""), "{stdout}");
    // The account id of the sample ARNs is never printed.
    assert!(!stdout.contains("123456789012"), "{stdout}");
}

#[test]
fn documented_shape_text_output_has_every_name_and_the_same_total() {
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", DOCUMENTED]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    for name in [
        "/aws/lambda/checkout",
        "/aws/ecs/app-service_v2.0#blue",
        "/aws/rds/instance/db-1/error",
        "/aws/delivery/flow",
        "/aws/lambda/worker",
    ] {
        assert!(stdout.contains(name), "{name}: {stdout}");
    }
    let total = stdout
        .lines()
        .find(|line| line.starts_with("total"))
        .expect("total line");
    assert!(
        total.contains("144955146240") && total.contains("4.05"),
        "{total}"
    );
    // 30 days retention on the 365-day-old checkout group already caps the horizon.
    assert!(!stdout.contains("advice /aws/lambda/checkout"), "{stdout}");
    assert!(
        stdout.contains("advice /aws/ecs/app-service_v2.0#blue: no retention;"),
        "{stdout}"
    );
}

#[test]
fn documented_pages_with_a_next_token_an_empty_page_and_a_directory_are_read() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("pages");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("page1.json"),
        r#"{"logGroups":[{"logGroupName":"/aws/lambda/a","storedBytes":1073741824,"retentionInDays":14,"metricFilterCount":0,"inheritedProperties":[]}],"nextToken":"opaque-token-1"}"#,
    )
    .unwrap();
    fs::write(
        dir.join("page2.json"),
        r#"{"logGroups":[{"logGroupName":"/aws/lambda/b","storedBytes":2147483648,"inheritedProperties":["ACCOUNT_DATA_PROTECTION","ACCOUNT_DATA_PROTECTION"]}]}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&["--json", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        json_field(&stdout, "estimated_usd_month"),
        ["0.03", "0.06", "0.09"]
    );
    assert!(stderr.contains("nextToken was not followed"), "{stderr}");
    assert!(!stderr.contains("opaque-token-1") && !stdout.contains("opaque-token-1"));
    // An empty page alone is not a bill: the tool says so with exit 2 instead of printing 0.00.
    let empty = scratch.path().join("empty.json");
    fs::write(&empty, r#"{"logGroups":[]}"#).unwrap();
    let (code, stdout, stderr) = run(&[empty.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
    assert!(stderr.contains("no log groups"), "{stderr}");
}

fn stale_and_partial(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let groups = scratch.path().join("partial.json");
    fs::write(
        &groups,
        r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1073741824}],"nextToken":"t"}"#,
    )
    .unwrap();
    (groups, stale_prices(scratch, "2000-01-01"))
}

#[test]
fn quiet_together_with_verbose_is_a_usage_error_with_exit_3() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    for args in [
        ["--quiet", "--verbose"],
        ["--verbose", "--quiet"],
        ["-q", "-v"],
    ] {
        let mut full = args.to_vec();
        full.push(groups.to_str().unwrap());
        let (code, stdout, stderr) = run(&full);
        assert_eq!(code, 3, "{args:?}: {stderr}");
        assert_eq!(stdout, "");
        assert!(
            stderr.starts_with("error: --quiet and --verbose cannot be used together"),
            "{stderr}"
        );
    }
}

#[test]
fn quiet_prints_only_the_result_and_hides_warnings() {
    let scratch = Scratch::new();
    let (groups, prices) = stale_and_partial(&scratch);
    let base = [
        "--as-of",
        "2026-11-09",
        "--prices",
        prices.to_str().unwrap(),
    ];
    let mut loud = base.to_vec();
    loud.push(groups.to_str().unwrap());
    let (_, loud_out, loud_err) = run(&loud);
    assert!(
        loud_out.contains("warning: ") && loud_out.contains("note: "),
        "{loud_out}"
    );
    assert!(loud_err.contains("price data dated"), "{loud_err}");
    let mut quiet = vec!["--quiet"];
    quiet.extend(base);
    quiet.push(groups.to_str().unwrap());
    let (code, stdout, stderr) = run(&quiet);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    assert!(
        stdout.starts_with(&format!("{ESTIMATE_LINE}\n")),
        "{stdout}"
    );
    assert!(
        stdout.contains("estimated_usd_month") && stdout.contains("total"),
        "{stdout}"
    );
    for hidden in [
        "warning:",
        "prices:",
        "as-of:",
        "free tier not applied",
        "not priced",
        "advice",
        "note:",
    ] {
        assert!(!stdout.contains(hidden), "{hidden}: {stdout}");
    }
    // The table is the same one the loud run prints.
    let table = |text: &str| -> Vec<String> {
        text.lines()
            .skip_while(|line| !line.starts_with("log_group"))
            .take_while(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    };
    assert_eq!(table(&stdout), table(&loud_out));
    assert_eq!(table(&stdout).len(), 3);
}

#[test]
fn quiet_keeps_json_whole_and_the_threshold_exit_code_and_still_reports_errors() {
    let scratch = Scratch::new();
    let (groups, prices) = stale_and_partial(&scratch);
    let g = groups.to_str().unwrap();
    let p = prices.to_str().unwrap();
    let (code, stdout, stderr) = run(&["--quiet", "--json", "--prices", p, g]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    assert!(
        stdout.contains("\"estimate\": true") && stdout.contains("\"warnings\": ["),
        "{stdout}"
    );
    assert!(stdout.contains("nextToken was not followed"), "{stdout}");
    let (code, stdout, stderr) = run(&["--quiet", "--fail-stored-bytes=1", "--prices", p, g]);
    assert_eq!((code, stderr.as_str()), (1, ""), "{stdout}");
    assert!(stdout.contains("threshold exceeded: yes"), "{stdout}");
    let missing = scratch.path().join("missing.json");
    let (code, stdout, stderr) = run(&["--quiet", missing.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert!(stderr.contains("missing.json"), "{stderr}");
}

#[test]
fn verbose_adds_progress_on_stderr_and_leaves_stdout_alone() {
    let scratch = Scratch::new();
    let groups = estimate_groups(&scratch);
    let g = groups.to_str().unwrap();
    let (code, plain_out, plain_err) = run(&["--as-of", "2026-11-09", g]);
    assert_eq!((code, plain_err.as_str()), (0, ""));
    let (code, stdout, stderr) = run(&["--verbose", "--as-of", "2026-11-09", g]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, plain_out);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr}");
    assert!(
        lines[0].starts_with("progress: reading ") && lines[0].ends_with("estimate.json"),
        "{stderr}"
    );
    assert_eq!(lines[1], "progress: priced 1 log group from 1 input");
    // JSON mode keeps stdout the document and puts the progress on stderr only.
    let (code, json_out, stderr) = run(&["--verbose", "--json", g]);
    assert_eq!(code, 0, "{stderr}");
    assert!(json_out.starts_with("{\n"), "{json_out}");
    assert!(stderr.contains("progress: reading "), "{stderr}");
    // The progress line escapes control characters of the path.
    // Windows refuses ESC in a file name, so there a different control character stands in.
    let control = if cfg!(windows) { '\u{7f}' } else { '\u{1b}' };
    let odd = scratch.path().join(format!("a{control}[2Jb.json"));
    fs::copy(&groups, &odd).unwrap();
    let (code, _, stderr) = run(&["--verbose", odd.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains(control), "{stderr:?}");
}

#[test]
fn help_lists_quiet_and_verbose() {
    let (code, stdout, _) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--quiet") && stdout.contains("--verbose"),
        "{stdout}"
    );
}

fn aged_groups(scratch: &Scratch) -> PathBuf {
    let path = scratch.path().join("aged.json");
    fs::write(
        &path,
        r#"{"logGroups":[{"logGroupName":"/a","storedBytes":492042190848,"creationTime":1762646400000},{"logGroupName":"/b","storedBytes":161730679452,"creationTime":1759622400000}]}"#,
    )
    .unwrap();
    path
}

fn md_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split(" | ")
        .map(|cell| cell.trim().to_string())
        .collect()
}

#[test]
fn markdown_table_has_the_amounts_of_the_text_and_the_json() {
    let scratch = Scratch::new();
    let path = aged_groups(&scratch);
    let g = path.to_str().unwrap();
    let (code, md, stderr) = run(&["--md", "--as-of", "2026-11-09", g]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    assert_eq!(md.matches(ESTIMATE_LINE).count(), 1, "{md}");
    let rows: Vec<&str> = md.lines().filter(|l| l.starts_with('|')).collect();
    assert_eq!(rows.len(), 5, "{md}");
    let header = md_cells(rows[0]);
    assert_eq!(header.len(), 10);
    assert_eq!(header[6], "estimated_usd_month");
    assert!(
        md_cells(rows[1])
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':')),
        "{}",
        rows[1]
    );
    let (_, text, _) = run(&["--as-of", "2026-11-09", g]);
    let (_, json, _) = run(&["--json", "--as-of", "2026-11-09", g]);
    let text_rows: Vec<Vec<&str>> = text
        .lines()
        .skip_while(|l| !l.starts_with("log_group"))
        .skip(1)
        .take(3)
        .map(|l| l.split_whitespace().collect())
        .collect();
    let mut json_money = Vec::new();
    for key in [
        "estimated_usd_month",
        "save_30_usd",
        "save_90_usd",
        "save_365_usd",
    ] {
        json_money.push(json_field(&json, key));
    }
    // Groups come first in name order, then the total, in all three formats.
    let md_rows = [rows[2], rows[3], rows[4]];
    for (i, md_row) in md_rows.iter().enumerate() {
        let cells = md_cells(md_row);
        let tokens = &text_rows[i];
        assert_eq!(cells[6..], tokens[tokens.len() - 4..], "row {i}");
        for (k, key_values) in json_money.iter().enumerate() {
            let json_index = if i < 2 { i } else { key_values.len() - 1 };
            assert_eq!(cells[6 + k], key_values[json_index], "row {i} column {k}");
        }
    }
    assert!(md_cells(rows[4])[0].contains("total"), "{}", rows[4]);
    // The total cell is the JSON total, not a re-rounded sum of the rows.
    assert_eq!(md_cells(rows[4])[6], json_money[0][2]);
}

#[test]
fn markdown_keeps_warnings_quiet_and_json_conflict_rules() {
    let scratch = Scratch::new();
    let (groups, prices) = stale_and_partial(&scratch);
    let g = groups.to_str().unwrap();
    let p = prices.to_str().unwrap();
    let (code, md, _) = run(&["--md", "--prices", p, g]);
    assert_eq!(code, 0);
    assert!(
        md.lines()
            .any(|l| l.starts_with("> warning: ") && l.contains("nextToken")),
        "{md}"
    );
    let (code, quiet, stderr) = run(&["--md", "--quiet", "--prices", p, g]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    assert!(
        !quiet.contains("warning") && quiet.contains(ESTIMATE_LINE),
        "{quiet}"
    );
    let (code, stdout, stderr) = run(&["--md", "--json", g]);
    assert_eq!((code, stdout.as_str()), (3, ""));
    assert!(
        stderr.starts_with("error: --json and --md cannot be used together"),
        "{stderr}"
    );
    let (_, help, _) = run(&["--help"]);
    assert!(help.contains("--md"), "{help}");
}

#[cfg(unix)]
mod sigint {
    use super::*;
    use std::io::{BufRead, BufReader, Read};

    /// Starts the tool on standard input (which stays open and empty), waits for the
    /// first progress line so the handler is surely installed, then sends a real SIGINT.
    fn interrupt(extra: &[&str]) -> (Option<i32>, String, String, Child) {
        let _guard = spawn_lock();
        let mut cmd = command(&["--verbose"]);
        cmd.args(extra);
        cmd.arg("-");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        assert_eq!(first, "progress: reading -\n");
        let status = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
        let mut rest = String::new();
        stderr.read_to_string(&mut rest).unwrap();
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        let code = child.wait().unwrap().code();
        (code, stdout, rest, child)
    }

    use std::process::Child;

    #[test]
    fn sigint_exits_130_with_one_line_and_no_output() {
        for extra in [&[][..], &["--json"][..], &["--md"][..]] {
            let (code, stdout, stderr, _child) = interrupt(extra);
            assert_eq!(code, Some(130), "{extra:?}: {stderr}");
            assert_eq!(stdout, "", "{extra:?}");
            assert_eq!(
                stderr, "cwlogs-retentie: interrupted by SIGINT\n",
                "{extra:?}"
            );
        }
    }
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn output_writes_the_report_to_a_file_and_replaces_an_existing_one_atomically() {
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let g = input.to_str().unwrap();
    for (flag, name) in [
        (None, "r.txt"),
        (Some("--json"), "r.json"),
        (Some("--md"), "r.md"),
    ] {
        let (_, expected, _) = match flag {
            Some(flag) => run(&[flag, "--as-of", "2026-11-09", g]),
            None => run(&["--as-of", "2026-11-09", g]),
        };
        let out = scratch.path().join(name);
        fs::write(&out, "old report\n").unwrap();
        let mut args = vec![
            "--output",
            out.to_str().unwrap(),
            "--as-of",
            "2026-11-09",
            g,
        ];
        args.extend(flag);
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!((stdout.as_str(), stderr.as_str()), ("", ""));
        assert_eq!(fs::read_to_string(&out).unwrap(), expected, "{name}");
    }
    // No temporary file is left next to the reports.
    assert_eq!(
        listing(scratch.path()),
        ["estimate.json", "r.json", "r.md", "r.txt"]
    );
}

#[test]
fn output_keeps_the_threshold_exit_code_and_still_writes_the_file() {
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let out = scratch.path().join("r.txt");
    let (code, stdout, stderr) = run(&[
        "--fail-stored-bytes=1",
        "--output",
        out.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert_eq!((code, stdout.as_str(), stderr.as_str()), (1, "", ""));
    assert!(fs::read_to_string(&out)
        .unwrap()
        .contains("threshold exceeded: yes"));
}

#[test]
fn output_is_not_touched_when_the_run_fails() {
    let scratch = Scratch::new();
    let out = scratch.path().join("r.txt");
    fs::write(&out, "old report\n").unwrap();
    let missing = scratch.path().join("missing.json");
    let (code, stdout, stderr) =
        run(&["--output", out.to_str().unwrap(), missing.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
    assert!(stderr.contains("missing.json"), "{stderr}");
    assert_eq!(fs::read_to_string(&out).unwrap(), "old report\n");
    assert_eq!(listing(scratch.path()), ["r.txt"]);
}

#[test]
fn output_write_errors_exit_2_with_a_clean_message() {
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let g = input.to_str().unwrap();
    // Missing parent directory: not created, exit 2, the odd name is escaped.
    let odd = scratch.path().join("no\u{1b}[2Jdir").join("r.txt");
    let (code, stdout, stderr) = run(&["--output", odd.to_str().unwrap(), g]);
    assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
    assert!(
        stderr.starts_with("cwlogs-retentie: could not write the report to "),
        "{stderr}"
    );
    assert!(
        !stderr.contains('\u{1b}') && !stderr.contains("panicked"),
        "{stderr:?}"
    );
    assert!(!scratch.path().join("no\u{1b}[2Jdir").exists());
    // A directory in the way is refused and left as it is.
    let dir = scratch.path().join("taken");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("keep"), "x").unwrap();
    let (code, stdout, stderr) = run(&["--output", dir.to_str().unwrap(), g]);
    assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
    assert!(
        stderr.starts_with("cwlogs-retentie: could not write the report to "),
        "{stderr}"
    );
    assert_eq!(listing(&dir), ["keep"]);
    assert_eq!(listing(scratch.path()), ["estimate.json", "taken"]);
}

#[cfg(unix)]
#[test]
fn output_refuses_to_replace_a_special_file_and_leaves_it_as_it_is() {
    use std::os::unix::fs::FileTypeExt;
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let pipe = scratch.path().join("pipe.out");
    let made = {
        let _guard = spawn_lock();
        Command::new("mkfifo").arg(&pipe).status().unwrap()
    };
    assert!(made.success());
    let (code, stdout, stderr) =
        run(&["--output", pipe.to_str().unwrap(), input.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
    assert!(
        stderr.starts_with("cwlogs-retentie: could not write the report to "),
        "{stderr}"
    );
    assert!(fs::symlink_metadata(&pipe).unwrap().file_type().is_fifo());
    assert_eq!(listing(scratch.path()), ["estimate.json", "pipe.out"]);
}

#[cfg(unix)]
#[test]
fn output_refuses_a_symlink_that_points_at_a_special_file_or_a_directory() {
    use std::os::unix::fs::FileTypeExt;
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let pipe = scratch.path().join("pipe.out");
    let made = {
        let _guard = spawn_lock();
        Command::new("mkfifo").arg(&pipe).status().unwrap()
    };
    assert!(made.success());
    let to_pipe = scratch.path().join("to-pipe.txt");
    symlink(&pipe, &to_pipe).unwrap();
    let dir = scratch.path().join("taken");
    fs::create_dir(&dir).unwrap();
    let to_dir = scratch.path().join("to-dir.txt");
    symlink(&dir, &to_dir).unwrap();
    let to_null = scratch.path().join("to-null.txt");
    symlink("/dev/null", &to_null).unwrap();
    for link in [&to_pipe, &to_dir, &to_null] {
        let (code, stdout, stderr) =
            run(&["--output", link.to_str().unwrap(), input.to_str().unwrap()]);
        assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
        assert!(
            stderr.starts_with("cwlogs-retentie: could not write the report to "),
            "{stderr}"
        );
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    }
    assert!(fs::symlink_metadata(&pipe).unwrap().file_type().is_fifo());
    assert_eq!(
        listing(scratch.path()),
        [
            "estimate.json",
            "pipe.out",
            "taken",
            "to-dir.txt",
            "to-null.txt",
            "to-pipe.txt"
        ]
    );
}

#[cfg(unix)]
#[test]
fn output_does_not_follow_a_symlink_and_replaces_the_link() {
    let scratch = Scratch::new();
    let input = estimate_groups(&scratch);
    let target = scratch.path().join("target.txt");
    fs::write(&target, "precious\n").unwrap();
    let link = scratch.path().join("link.txt");
    symlink(&target, &link).unwrap();
    let (code, _, stderr) = run(&["--output", link.to_str().unwrap(), input.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(fs::read_to_string(&target).unwrap(), "precious\n");
    assert!(!fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(fs::read_to_string(&link)
        .unwrap()
        .contains("estimated_usd_month"));
    // A dangling link is replaced too, and nothing is created behind it.
    let dangling = scratch.path().join("dangling.txt");
    symlink(scratch.path().join("nowhere.txt"), &dangling).unwrap();
    let (code, _, stderr) = run(&[
        "--output",
        dangling.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!scratch.path().join("nowhere.txt").exists());
    assert!(!fs::symlink_metadata(&dangling)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn help_lists_output() {
    let (_, help, _) = run(&["--help"]);
    assert!(help.contains("--output <FILE>"), "{help}");
}

fn assert_schema_version(text: &str, what: &str) {
    let json: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: {e}: {text}"));
    assert_eq!(json["schema_version"], serde_json::json!(1), "{what}");
    // The version is the first key, so a reader sees it before anything else.
    assert!(
        text.starts_with("{\n  \"schema_version\": 1,\n"),
        "{what}: {text}"
    );
}

#[test]
fn every_json_output_path_has_schema_version_one() {
    let scratch = Scratch::new();
    let groups = aged_groups(&scratch);
    let g = groups.to_str().unwrap();
    let dir = scratch.path().join("pages");
    fs::create_dir(&dir).unwrap();
    fs::copy(&groups, dir.join("one.json")).unwrap();
    let prices = stale_prices(&scratch, "2000-01-01");
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("plain", vec!["--json", g]),
        ("as-of", vec!["--json", "--as-of", "2026-11-09", g]),
        ("quiet", vec!["--json", "--quiet", g]),
        ("verbose", vec!["--json", "--verbose", g]),
        ("directory", vec!["--json", dir.to_str().unwrap()]),
        (
            "prices file",
            vec!["--json", "--prices", prices.to_str().unwrap(), g],
        ),
        (
            "threshold exit 1",
            vec!["--json", "--fail-stored-bytes=1", g],
        ),
    ];
    for (what, args) in cases {
        let (code, stdout, stderr) = run(&args);
        assert!(code == 0 || code == 1, "{what}: {code} {stderr}");
        assert_schema_version(&stdout, what);
    }
    let (code, stdout, stderr) = run_stdin(&["--json", "-"], &fs::read(&groups).unwrap());
    assert_eq!(code, 0, "{stderr}");
    assert_schema_version(&stdout, "stdin");
    let out = scratch.path().join("r.json");
    let (code, _, stderr) = run(&["--json", "--output", out.to_str().unwrap(), g]);
    assert_eq!(code, 0, "{stderr}");
    assert_schema_version(&fs::read_to_string(&out).unwrap(), "--output");
}

#[test]
fn json_error_paths_print_no_json_document_and_no_schema_version() {
    let scratch = Scratch::new();
    let missing = scratch.path().join("missing.json");
    let (code, stdout, stderr) = run(&["--json", missing.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert!(!stderr.contains("schema_version"), "{stderr}");
    let (code, stdout, _) = run(&["--json", "--md", missing.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (3, ""));
    let bad = scratch.path().join("bad.json");
    fs::write(&bad, "{").unwrap();
    let (code, stdout, _) = run(&["--json", bad.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (2, ""));
}
