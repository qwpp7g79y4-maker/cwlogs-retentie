# Contributing

Thanks for your interest in cwlogs-retention.

## Building and testing

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

These checks must pass before a change is merged. The code builds with Rust 1.85 or newer, so run the same clippy and test commands with `cargo +1.85` when you can. Test fixtures must be synthetic, or anonymised copies of a public post. Never add a real account id, a real log group name, a real token, or a real `describe-log-groups` export.

A change to behaviour comes with a test that fails before the change and passes after it, and with the README lines that describe it. The test counts in `README.md`, `README.nl.md`, and `QUALITY.md` are checked against the sources by `tests/readme.rs`, so update them in the same change.

## Commit messages

Commit messages are short English sentences that say what changed, for example `Escape controls in log group names shown on the report`.

## Contact

PEPAI, pep_ai@icloud.com
