//! Handbook check against the built `cwlogs-retention` binary.
//!
//! `--help` is the usage text. `man/cwlogs-retention.1` must contain it
//! verbatim, its long options must be exactly those in `--help`, and it
//! must have the standard sections.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

/// Long options: a `--` at a token boundary plus the following name.
fn long_options(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut opts = BTreeSet::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && (i == 0 || !is_name_byte(bytes[i - 1])) {
            let start = i;
            i += 2;
            let name_at = i;
            while i < bytes.len() && is_name_byte(bytes[i]) {
                i += 1;
            }
            if i > name_at {
                opts.insert(text[start..i].to_string());
            }
            continue;
        }
        i += 1;
    }
    opts
}

#[test]
fn help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic() {
    let bin = env!("CARGO_BIN_EXE_cwlogs-retention");
    let output = Command::new(bin).arg("--help").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {:?}", output.stderr);
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(!help.is_empty(), "--help stdout is empty");

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("man/cwlogs-retention.1");
    let man = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read man page: {e}"));
    for section in [
        "NAME",
        "SYNOPSIS",
        "DESCRIPTION",
        "OPTIONS",
        "EXIT STATUS",
        "EXAMPLES",
    ] {
        assert!(
            man.contains(&format!(".SH {section}")),
            "man page is missing .SH {section}"
        );
    }
    assert!(
        man.contains(&help),
        "man page SYNOPSIS must contain the --help stdout verbatim"
    );

    let from_help = long_options(&help);
    let from_man = long_options(&man);
    assert!(!from_help.is_empty(), "help text has no long options");
    assert_eq!(
        from_man, from_help,
        "long options in man/cwlogs-retention.1 must be exactly those in --help"
    );

    // Each option is documented in the OPTIONS section itself, not only in the
    // verbatim usage text of the SYNOPSIS.
    let options_section = man
        .split(".SH OPTIONS")
        .nth(1)
        .and_then(|rest| rest.split("\n.SH ").next())
        .expect("OPTIONS section");
    assert_eq!(
        long_options(options_section),
        from_help,
        "the OPTIONS section of man/cwlogs-retention.1 must describe exactly the long options of --help"
    );

    let unknown = Command::new(bin)
        .arg("--handbook-unknown")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(
        !stderr.contains("panicked"),
        "unknown option panicked: {stderr}"
    );
}
