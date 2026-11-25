//! The files that go into the package must not carry local paths, internal wrapper
//! options, internal tool or folder names, internal file names, or names of AI systems.
//! Read: README, QUALITY, CHANGELOG, SECURITY, the licenses, `man/`, `Cargo.toml`,
//! `deny.toml`, `.gitignore`, `.gitattributes`, `.github/` (when present), `src/`,
//! `examples/`, `prices/`, and `tests/` except this file.
//!
//! The test also runs in the unpacked `.crate`, where cargo has rewritten `Cargo.toml`
//! (the `exclude` list is spread over several lines) and where `.github/` may be absent.
//!
//! The patterns are put together from pieces, so this source does not hold the
//! names it looks for.

use std::fs;
use std::path::{Path, PathBuf};

const INTERNAL_FILE_NAME: &str = "internal file name";

fn forbidden() -> Vec<(String, String)> {
    let pattern = |label: &str, parts: &[&str]| (label.to_string(), parts.concat());
    vec![
        pattern("local absolute path", &["/me", "dia/"]),
        pattern("local absolute path", &["/ho", "me/"]),
        pattern("home-relative path", &["~", "/"]),
        pattern("internal wrapper option", &["--max", "-gb"]),
        pattern("internal wrapper option", &["--na", "am"]),
        pattern(INTERNAL_FILE_NAME, &["ta", "sk.md"]),
        pattern(INTERNAL_FILE_NAME, &["blue", "print.md"]),
        pattern("internal tool name", &["geheugen", "wachter"]),
        pattern("internal tool name", &["brug", "wachter"]),
        pattern("internal tool name", &["brug", "-prompts"]),
        pattern("internal tool name", &["open", "bot"]),
        pattern("internal folder name", &["bouw", "-tmp"]),
        pattern("internal folder name", &["~", "/brug"]),
        pattern("internal word", &["pe", "pijn"]),
        pattern("AI system name", &["gr", "ok"]),
        pattern("AI system name", &["cla", "ude"]),
        pattern("AI system name", &["anthr", "opic"]),
        pattern("AI system name", &["son", "net"]),
        pattern("AI system name", &["op", "us"]),
        pattern("AI trailer", &["co-auth", "ored"]),
    ]
}

/// The two files that stay out of git and out of the package, as `/name` entries.
fn ignored_entries() -> [String; 2] {
    [["/blue", "print.md"].concat(), ["/ta", "sk.md"].concat()]
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(path)
            .unwrap_or_else(|e| panic!("read dir {}: {e}", path.display()))
            .map(|entry| entry.expect("entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else {
        out.push(path.to_path_buf());
    }
}

/// Files below `path`, or none when the directory does not exist (`.github/` is not in
/// every packaged copy).
fn collect_if_present(path: &Path, out: &mut Vec<PathBuf>) {
    if path.exists() {
        collect(path, out);
    }
}

/// The patterns found in `text`, each with its 1-based line number.
fn hits(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for (label, pattern) in forbidden() {
            if lower.contains(&pattern) {
                found.push((index + 1, format!("{label} `{pattern}`")));
            }
        }
    }
    found
}

/// 1-based numbers of the lines of a `Cargo.toml` that belong to the `exclude` list: the
/// line `exclude = [...]` itself, or all lines from `exclude = [` to the closing `]`
/// (cargo writes the list that way into the packaged manifest).
fn exclude_lines(manifest: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut open = false;
    for (index, line) in manifest.lines().enumerate() {
        let trimmed = line.trim_start();
        if !open && trimmed.starts_with("exclude") && trimmed.contains('=') {
            open = true;
        }
        if open {
            lines.push(index + 1);
            let after_open = trimmed.split_once('[').map_or(trimmed, |(_, rest)| rest);
            if after_open.contains(']') {
                open = false;
            }
        }
    }
    lines
}

/// What is wrong in one file. The `exclude` list of `Cargo.toml` and the two entries of
/// `.gitignore` may name the two ignored files; nothing else is allowed there either.
fn problems_in(file_name: &str, text: &str) -> Vec<(usize, String)> {
    let allowed_names: Vec<usize> = match file_name {
        "Cargo.toml" => exclude_lines(text),
        ".gitignore" => text
            .lines()
            .enumerate()
            .filter(|(_, line)| ignored_entries().iter().any(|entry| entry == line.trim()))
            .map(|(index, _)| index + 1)
            .collect(),
        _ => Vec::new(),
    };
    hits(text)
        .into_iter()
        .filter(|(line, what)| {
            !(allowed_names.contains(line) && what.starts_with(INTERNAL_FILE_NAME))
        })
        .collect()
}

/// The entries of the ignored-files list that `manifest` does not carry in its `exclude`.
fn missing_excludes(manifest: &str) -> Vec<String> {
    let lines = exclude_lines(manifest);
    let block: Vec<&str> = manifest
        .lines()
        .enumerate()
        .filter(|(index, _)| lines.contains(&(index + 1)))
        .map(|(_, line)| line)
        .collect();
    ignored_entries()
        .into_iter()
        .filter(|entry| {
            let quoted = format!("\"{entry}\"");
            !block.iter().any(|line| line.contains(&quoted))
        })
        .collect()
}

#[test]
fn packaged_files_have_no_local_paths_internal_names_or_ai_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = [
        "README.md",
        "README.nl.md",
        "QUALITY.md",
        "QUALITY.nl.md",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "build.rs",
        "about.hbs",
        "about.toml",
        "THIRD-PARTY-LICENSES.html",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "Cargo.toml",
        "deny.toml",
    ]
    .iter()
    .map(|name| root.join(name))
    .collect();
    for optional in [".gitignore", ".gitattributes"] {
        if root.join(optional).exists() {
            files.push(root.join(optional));
        }
    }
    let before = files.len();
    for dir in ["man", "src", "examples", "prices", "tests"] {
        let found = files.len();
        collect(&root.join(dir), &mut files);
        assert!(files.len() > found, "{dir}/ holds no file");
    }
    collect_if_present(&root.join(".github"), &mut files);
    assert!(files.len() > before + 4);
    files.retain(|file| !file.ends_with("tests/docs_clean.rs"));
    let mut problems = Vec::new();
    for file in &files {
        let text =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        for (line, what) in problems_in(name, &text) {
            problems.push(format!("{}:{line}: {what}", file.display()));
        }
    }
    assert!(
        problems.is_empty(),
        "internal text in packaged files:\n{}",
        problems.join("\n")
    );
}

#[test]
fn cargo_toml_keeps_both_ignored_files_out_of_the_package() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let missing = missing_excludes(&manifest);
    assert!(
        missing.is_empty(),
        "not in `exclude` of Cargo.toml: {missing:?}"
    );
    let ignore = root.join(".gitignore");
    if ignore.exists() {
        let text = fs::read_to_string(ignore).unwrap();
        for entry in ignored_entries() {
            assert!(
                text.lines().any(|line| line.trim() == entry),
                "{entry} is not in .gitignore"
            );
        }
    }
}

#[test]
fn the_exclude_list_may_be_one_line_or_the_block_cargo_writes() {
    let [first, second] = ignored_entries();
    let one_line = format!("[package]\nexclude = [\"{first}\", \"{second}\"]\nname = \"x\"\n");
    let block = format!("[package]\nbuild = false\nexclude = [\n    \"{first}\",\n    \"{second}\",\n]\nautolib = false\n");
    for manifest in [&one_line, &block] {
        assert!(problems_in("Cargo.toml", manifest).is_empty(), "{manifest}");
        assert!(missing_excludes(manifest).is_empty(), "{manifest}");
    }
    // The block ends at its closing bracket: the same name below it is a finding.
    let after = format!("{block}note = \"{first}\"\n");
    assert_eq!(problems_in("Cargo.toml", &after).len(), 1, "{after}");
    // Another pattern inside the list is a finding too.
    let local = ["/ho", "me/x"].concat();
    let bad = format!("exclude = [\n    \"{first}\",\n    \"{local}\",\n]\n");
    assert_eq!(problems_in("Cargo.toml", &bad).len(), 1, "{bad}");
}

#[test]
fn a_path_missing_from_exclude_is_found() {
    let [first, second] = ignored_entries();
    assert_eq!(
        missing_excludes("exclude = []\n"),
        vec![first.clone(), second.clone()]
    );
    assert_eq!(
        missing_excludes(&format!("exclude = [\n    \"{first}\",\n]\n")),
        vec![second.clone()]
    );
    // A name outside the list does not count as excluded.
    assert_eq!(
        missing_excludes(&format!(
            "exclude = []\ndescription = \"{first} {second}\"\n"
        ))
        .len(),
        2
    );
}

#[test]
fn only_the_two_entries_may_stand_in_gitignore() {
    let [first, second] = ignored_entries();
    assert!(problems_in(".gitignore", &format!("/target\n{first}\n{second}\n")).is_empty());
    assert_eq!(problems_in(".gitignore", &format!("# {first}\n")).len(), 1);
    assert_eq!(problems_in("README.md", &format!("{first}\n")).len(), 1);
}

#[test]
fn a_missing_directory_is_skipped_only_where_it_is_optional() {
    let mut files = Vec::new();
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-directory");
    collect_if_present(&missing, &mut files);
    assert!(files.is_empty());
}

#[test]
fn the_check_itself_finds_each_pattern_and_passes_plain_text() {
    for (label, pattern) in forbidden() {
        let text = format!("fine line\nbefore {pattern} after\n");
        let found = hits(&text);
        assert!(
            found
                .iter()
                .any(|(line, what)| *line == 2 && what.contains(&label)),
            "{pattern}: {found:?}"
        );
    }
    assert!(
        hits("plain text, `cargo test --locked`, examples/describe-log-groups.json").is_empty()
    );
}

#[test]
fn this_source_does_not_hold_the_names_it_looks_for() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/docs_clean.rs"))
            .unwrap()
            .replace("\r\n", "\n");
    let found = hits(&source);
    assert!(found.is_empty(), "{found:?}");
}
