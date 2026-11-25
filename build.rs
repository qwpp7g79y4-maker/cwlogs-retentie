//! Debug builds print the git commit of this tree after the crate version
//! in `--version`. Release builds and builds without git, or where git does
//! not give a full 40-digit commit, print the crate version alone.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(cwlogs_retentie_git_sha)");
    println!("cargo:rerun-if-changed=build.rs");
    watch_head();
    if std::env::var("PROFILE").as_deref() != Ok("debug") {
        return;
    }
    if let Some(sha) = git(&["rev-parse", "--verify", "HEAD"]).filter(|sha| is_commit(sha)) {
        println!("cargo:rustc-cfg=cwlogs_retentie_git_sha");
        println!("cargo:rustc-env=CWLOGS_RETENTIE_GIT_SHA={sha}");
    }
}

fn is_commit(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git(args: &[&str]) -> Option<String> {
    let dir =
        std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

/// Rebuild when the commit changes: `HEAD` itself and the branch file it names.
fn watch_head() {
    for name in ["HEAD", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            if PathBuf::from(&path).is_file() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &branch]) {
            if PathBuf::from(&path).is_file() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
}
