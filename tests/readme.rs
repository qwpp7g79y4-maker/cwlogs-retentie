//! README.md is executable: console blocks match the binary, and every
//! numbered claim has a test.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
        .replace("\r\n", "\n")
}

fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("missing heading {heading}"))
        + marker.len();
    let rest = &text[start..];
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

fn rust_sources() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::new();
    for dir in ["src", "tests"] {
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push_str(&fs::read_to_string(&path).unwrap());
                out.push('\n');
            }
        }
    }
    out
}

/// Every ```console block: the first line is `$ cwlogs-retention ARGS`, the rest is stdout.
fn console_blocks(doc: &str) -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line != "```console" {
            continue;
        }
        let call = lines.next().expect("empty console block");
        let args: Vec<String> = call
            .strip_prefix("$ cwlogs-retention ")
            .unwrap_or_else(|| {
                panic!("console block does not start with `$ cwlogs-retention `: {call}")
            })
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let mut body = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "```" {
                closed = true;
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        assert!(closed, "console block not closed: {call}");
        out.push((call.to_string(), args, body));
    }
    assert!(!out.is_empty(), "README has no console block");
    out
}

const EN_HEADINGS: [&str; 13] = [
    "Installation",
    "Quick start",
    "Quickstart in 5 minutes",
    "Options and usage",
    "Example",
    "How it counts",
    "Exit codes",
    "Limitations",
    "Prices and responsibility",
    "Maintenance",
    "Support",
    "License",
    "Contact",
];

fn headings(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| line.trim_start_matches("## "))
        .collect()
}

#[test]
fn readme_sections_follow_the_documentation_standard() {
    let text = read("README.md");
    assert_eq!(headings(&text), EN_HEADINGS.to_vec());
    assert!(!text.contains("## PEPAI"));
}

#[test]
fn the_boundaries_come_before_installation() {
    let text = read("README.md");
    let head = &text[..text.find("\n## Installation\n").unwrap()];
    for phrase in ["no network", "writes nothing", "no account", "estimate"] {
        assert!(
            head.contains(phrase),
            "boundaries before Installation: {phrase}"
        );
    }
}

#[test]
fn installation_uses_a_local_path() {
    let readme = read("README.md");
    let install = section(&readme, "## Installation");
    assert!(install.contains("cargo install --locked --path ."));
    assert!(install.contains("1.85"));
    assert!(install.contains("not published"));
    assert!(install.contains("release.yml"));
}

#[test]
fn quick_start_reads_the_sample_and_writes_nothing() {
    for (file, heading) in [
        ("README.md", "## Quick start"),
        ("README.nl.md", "## Snelstart"),
    ] {
        let text = read(file);
        let quick = section(&text, heading);
        assert!(quick.contains(
            "$ cwlogs-retention --quiet --as-of 2026-11-09 examples/describe-log-groups.json"
        ));
        for write in ["cat >", "tee ", "> examples", ">> ", "<<"] {
            assert!(!quick.contains(write), "{file} quick start writes: {write}");
        }
    }
    assert!(read("examples/describe-log-groups.json").contains("123456789012"));
}

#[test]
fn usage_block_is_the_help_text() {
    let out = Command::new(env!("CARGO_BIN_EXE_cwlogs-retention"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(out.stdout).unwrap();
    for (file, heading) in [
        ("README.md", "## Options and usage"),
        ("README.nl.md", "## Opties en gebruik"),
    ] {
        let text = read(file);
        let usage = section(&text, heading);
        let block = format!("```text\n{help}```\n");
        assert!(
            usage.contains(&block),
            "{file} usage block differs from --help"
        );
    }
}

#[test]
fn the_dutch_readme_has_the_same_content() {
    let en = read("README.md");
    let nl = read("README.nl.md");
    assert_eq!(
        headings(&nl),
        vec![
            "Installatie",
            "Snelstart",
            "Aan de slag in 5 minuten",
            "Opties en gebruik",
            "Voorbeeld",
            "Hoe het telt",
            "Afsluitcodes",
            "Beperkingen",
            "Prijzen en verantwoordelijkheid",
            "Onderhoud",
            "Ondersteuning",
            "Licentie",
            "Contact",
        ]
    );
    let blocks = |doc: &str| -> Vec<(String, String)> {
        console_blocks(doc)
            .into_iter()
            .map(|(c, _, b)| (c, b))
            .collect()
    };
    assert_eq!(blocks(&en), blocks(&nl), "console blocks differ");
    let claims = |doc: &str, heading: &str| -> Vec<String> {
        section(doc, heading)
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix('C')?;
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                (!digits.is_empty() && rest[digits.len()..].starts_with('.'))
                    .then(|| format!("C{digits}"))
            })
            .collect()
    };
    assert_eq!(
        claims(&en, "## How it counts"),
        claims(&nl, "## Hoe het telt")
    );
    assert_eq!(claims(&nl, "## Hoe het telt").len(), 24);
    for (file, doc) in [("README.md", &en), ("README.nl.md", &nl)] {
        for code in ["0", "1", "2", "3", "130"] {
            let codes = section(
                doc,
                if file == "README.md" {
                    "## Exit codes"
                } else {
                    "## Afsluitcodes"
                },
            );
            assert!(
                codes.contains(&format!("| {code} |")),
                "{file}: exit code {code}"
            );
        }
    }
}

#[test]
fn both_readmes_end_with_the_contact_section() {
    for file in ["README.md", "README.nl.md"] {
        let text = read(file);
        let contact = &text[text
            .find("\n## Contact\n")
            .unwrap_or_else(|| panic!("{file}: no Contact"))..];
        for item in [
            "pep_ai@icloud.com",
            "https://www.pepai.nl",
            "https://x.com/pepai_nl",
            "https://ko-fi.com/pepai",
        ] {
            assert!(contact.contains(item), "{file}: Contact lacks {item}");
        }
        assert!(
            !contact[1..].contains("\n## "),
            "{file}: Contact is not last"
        );
    }
}

#[test]
fn the_examples_folder_documents_each_command_and_its_expected_output() {
    let readme = read("examples/README.md");
    assert!(readme.contains("Expected exit code: 0"));
    let expected = read("examples/describe-log-groups.expected.txt");
    let command = "cwlogs-retention --as-of 2026-11-09 examples/describe-log-groups.json";
    assert!(readme.contains(&format!("```sh\n{command}\n```")));
    assert!(readme.contains("examples/describe-log-groups.expected.txt"));
    let out = Command::new(env!("CARGO_BIN_EXE_cwlogs-retention"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CWLOGS_RETENTION_TODAY", "2026-11-20")
        .args(["--as-of", "2026-11-09", "examples/describe-log-groups.json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), expected);
}

#[test]
fn console_examples_match_the_binary() {
    let readme = read("README.md");
    for (call, args, expected) in console_blocks(&readme) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_cwlogs-retention"));
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
        cmd.env("NO_COLOR", "1");
        cmd.env("CWLOGS_RETENTION_TODAY", "2026-11-20");
        cmd.args(&args);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(out.status.success(), "{call} failed: {stderr}");
        assert!(stderr.is_empty(), "{call} wrote to stderr: {stderr}");
        assert_eq!(stdout, expected, "console mismatch for {call}");
        assert!(!stdout.contains("123456789012"));
    }
}

#[test]
fn every_how_it_counts_claim_has_a_test() {
    let readme = read("README.md");
    let how = section(&readme, "## How it counts");
    let mut ids = Vec::new();
    for line in how.lines() {
        let Some(rest) = line.trim().strip_prefix('C') else {
            continue;
        };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        ids.push(digits.parse::<u32>().unwrap());
    }
    assert_eq!(ids, (1..=24).collect::<Vec<_>>());
    let sources = rust_sources();
    for id in 1..=24 {
        let needle = format!("fn claim_c{id}_");
        assert!(sources.contains(&needle), "missing {needle}");
    }
}

#[test]
fn limitations_name_behaviors_the_tests_cover() {
    let readme = read("README.md");
    let limits = section(&readme, "## Limitations");
    for phrase in [
        "put-retention-policy",
        "not executed",
        "not a forecast",
        "AWS SDK",
        "no network",
        "nextToken",
        "268435456",
        "symlink",
        "directory is nested too deeply",
        "per day",
        "free tier",
        "us-east-1",
    ] {
        assert!(limits.contains(phrase), "limitations missing {phrase}");
    }
    let sources = rust_sources();
    for needle in [
        "fn file_larger_than_268435456_bytes",
        "fn directory_symlink_is_skipped",
        "fn explicit_symlink_is_followed",
        "fn directory_at_64_levels_is_accepted",
        "fn directory_deeper_than_64_levels_is_an_error",
        "fn claim_c10_",
        "fn claim_c16_",
        "fn claim_c17_",
        "fn claim_c21_",
        "fn claim_c24_",
    ] {
        assert!(sources.contains(needle), "missing {needle}");
    }
}

#[test]
fn licenses_and_changelog_match_the_package() {
    let mit = read("LICENSE-MIT");
    assert!(mit.contains("Copyright (c) 2026 PEPAI"));
    let apache = read("LICENSE-APACHE");
    assert!(apache.contains("Copyright 2026 PEPAI"));
    let changelog = read("CHANGELOG.md");
    assert!(changelog.contains("## [1.0.0] - 2026-11-15"));
    assert!(changelog.contains("## [0.1.0] - 2026-11-09"));
    assert!(changelog.contains("not published"));
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("MIT OR Apache-2.0"));
    assert!(cargo.contains("rust-version = \"1.85\""));
    assert!(!cargo.contains("[dev-dependencies]"));
    let readme = read("README.md");
    let license = section(&readme, "## License");
    assert!(license.contains("MIT OR Apache-2.0"));
    assert!(license.contains("LICENSE-MIT"));
    assert!(license.contains("LICENSE-APACHE"));
}

/// Number of test attributes in each source group, counted from the files
/// (tests that run only on some systems count too).
fn test_counts() -> [(&'static str, usize); 6] {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let count_in = |path: PathBuf| {
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count()
    };
    let unit: usize = fs::read_dir(root.join("src"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .map(count_in)
        .sum();
    let file = |name: &str| count_in(root.join("tests").join(name));
    [
        ("unit", unit),
        ("claims", file("claims.rs")),
        ("CLI", file("cli.rs")),
        ("docs", file("docs_clean.rs")),
        ("handbook", file("handbook_help.rs")),
        ("README", file("readme.rs")),
    ]
}

#[test]
fn test_counts_in_the_readme_and_quality_match_the_sources() {
    let counts = test_counts();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let parts: Vec<String> = counts
        .iter()
        .map(|(name, n)| format!("{n} {name}"))
        .collect();
    let sentence = format!("runs {total} tests ({})", parts.join(", "));
    let readme = read("README.md");
    assert!(readme.contains(&sentence), "README should say: {sentence}");
    let nl_parts: Vec<String> = counts
        .iter()
        .map(|(name, n)| format!("{n} {}", name))
        .collect();
    let nl_sentence = format!("voert {total} tests uit ({})", nl_parts.join(", "));
    assert!(
        read("README.nl.md").contains(&nl_sentence),
        "README.nl.md should say: {nl_sentence}"
    );
    let quality = read("QUALITY.md");
    assert!(
        quality.contains(&format!("Suite: {total} passed")),
        "QUALITY.md should say: Suite: {total} passed"
    );
    for (file, n) in [
        ("tests/claims.rs", counts[1].1),
        ("tests/cli.rs", counts[2].1),
        ("tests/docs_clean.rs", counts[3].1),
        ("tests/handbook_help.rs", counts[4].1),
        ("tests/readme.rs", counts[5].1),
    ] {
        let row = format!("| `{file}` | {n} | 0 | 0 |");
        assert!(quality.contains(&row), "QUALITY.md should have: {row}");
    }
    assert!(
        quality.contains(&format!(
            "| unittests `src/lib.rs` | {} | 0 | 0 |",
            counts[0].1
        )),
        "QUALITY.md unit row"
    );
    assert!(
        quality.contains(&format!("| total | {total} | 0 | 0 |")),
        "QUALITY.md total row"
    );
}

#[test]
fn quality_claims_table_names_a_test_for_every_claim() {
    let sources = rust_sources();
    for file in ["QUALITY.md", "QUALITY.nl.md"] {
        let quality = read(file);
        for id in 1..=24 {
            let row = quality
                .lines()
                .find(|line| line.starts_with(&format!("| C{id} |")))
                .unwrap_or_else(|| panic!("{file}: no row for C{id}"));
            let name = row
                .split('|')
                .nth(3)
                .and_then(|cell| cell.split('`').nth(1))
                .unwrap_or_else(|| panic!("{file}: C{id} names no test"));
            assert!(
                name.starts_with(&format!("claim_c{id}_")),
                "{file}: C{id} row names {name}"
            );
            assert!(
                sources.contains(&format!("fn {name}(")),
                "{file}: no test {name}"
            );
        }
    }
}

#[test]
fn quality_names_the_tested_commit_the_date_and_what_was_not_run() {
    for (file, not_run) in [
        ("QUALITY.md", "Not run"),
        ("QUALITY.nl.md", "Niet uitgevoerd"),
    ] {
        let quality = read(file);
        assert!(quality.contains(not_run), "{file}: no `{not_run}` list");
        for page in ["API_DescribeLogGroups.html", "API_LogGroup.html"] {
            let url = format!(
                "https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/{page}"
            );
            assert!(quality.contains(&url), "{file}: missing {url}");
        }
        let has_date = quality.as_bytes().windows(10).any(|w| {
            w.iter().enumerate().all(|(i, b)| match i {
                4 | 7 => *b == b'-',
                _ => b.is_ascii_digit(),
            })
        });
        assert!(has_date, "{file}: no date");
        let has_commit = quality
            .split(|c: char| !c.is_ascii_hexdigit())
            .any(|word| word.len() == 40 || word.len() == 7);
        assert!(has_commit, "{file}: no commit hash");
    }
}
