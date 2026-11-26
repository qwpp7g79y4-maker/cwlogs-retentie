# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-15

Changes made after 2026-11-15; the version number did not change.

### Added

- `THIRD-PARTY-LICENSES.html`: the licenses of the third-party crates in the binary, generated with `cargo about generate --locked about.hbs`.
- `README.nl.md`, `QUALITY.nl.md`, and `CONTRIBUTING.md`. The README now follows the documentation standard: boundaries first, Installation, Quick start (reads `examples/describe-log-groups.json`, writes nothing), a Dutch five-minute route, Options and usage (the `--help` text, checked by a test), Example, How it counts, Exit codes, Limitations, Prices and responsibility, Maintenance, Support, License, and Contact.
- `examples/README.md` and `examples/describe-log-groups.expected.txt`; a test runs the command and compares the output.
- `build.rs`: a debug build prints the git commit after the version in `--version`; a release build, or a build without git, prints the version alone.
- `about.hbs` and `about.toml` for the third-party license list, and `.github/workflows/release.yml`, which builds the archives for five targets from a `v*` tag. No tag exists yet.
- `QUALITY.md` has a table of each claim C1 to C24 and the test that covers it; tests check that the tables exist and name real tests.
- `"schema_version": 1` as the first key of every JSON document.
- `--output FILE` writes the report to a file through a temporary file and a rename; a symlink at `FILE` is replaced and not followed; a failed write is exit 2 with a message.
- SIGINT (Unix) ends the run with exit 130 and `cwlogs-retention: interrupted by SIGINT` on standard error, with nothing on standard output. `src/interrupt.rs` is the second binary-only `unsafe` file.
- `--md` prints the table as Markdown with the same amounts, the estimate line, and escaped cells; `--md` with `--json` is a usage error with exit 3.
- `--quiet` (`-q`) and `--verbose` (`-v`). Quiet keeps the estimate line, the table, and the threshold lines in text and hides warnings; verbose adds `progress:` lines on standard error; both together exit 3.
- `man/cwlogs-retention.1` holds the `--help` text and the long options, and `tests/handbook_help.rs` checks both against the binary.
- `escape_os_str` in the library, which writes a path or an argument as one line of text.
- `SECURITY.md` with the reporting address and the supported versions.
- `tests/docs_clean.rs` fails when README, QUALITY, CHANGELOG, SECURITY, the licenses, the man page, `Cargo.toml`, `deny.toml`, `.gitignore`, `.gitattributes`, `.github/`, `src/`, `examples/`, `prices/`, or `tests/` (except that file) holds a local path, an internal tool or folder name or option, an internal file name, or the name of an AI system. Only the `exclude` list of `Cargo.toml` and the two entries of `.gitignore` may name the two ignored files, and the test fails when `Cargo.toml` no longer lists both in `exclude`. It also passes in the unpacked `.crate`, where cargo writes the `exclude` list over several lines and `.github/` may be absent. A second check in `tests/readme.rs` compares the test counts in README and QUALITY with the sources.
- README sections "Input rules" and "Size, time, and system" with the measured limits: nesting depth, retention range, price digits, directory members, time and memory at the file size limit, and the systems tried.
- `.github/dependabot.yml`, `timeout-minutes` for every CI job (30 for the test, Rust 1.85, and package jobs, 15 for the audit and license jobs), `workflow_call`, and `persist-credentials: false` for checkouts.
- A CI job `package` that runs `cargo package`, unpacks the `.crate`, and runs `cargo test` in it.
- README "Money precision" and three more input rules (a number given as a string is at most 20 characters, `storedBytes` and the total of all groups fit in 18446744073709551615, each differing ARN region is one warning), each measured with the binary and each with a test.
- `SECURITY.md` names the memory use and the last-cent limit as documented limits.
- More unit and CLI tests for the changes below, bringing `cargo test --locked` to 219 tests, among them 13 that run only on Linux for closed, full and broken standard output and for arguments and file names that need Linux.

### Changed

- The crate, the binary, and the repository are named `cwlogs-retention`. The clock override is `CWLOGS_RETENTION_TODAY`.
- An entry that holds both `arn` and `logGroupArn`, as the API documents, is accepted when both name the same region. Before, it was refused with `both arn and logGroupArn`; two different regions are still refused. `tests/fixtures/describe-log-groups-documented.json` and three CLI tests cover the documented input shape.
- A warning on standard error when the date of the price data is more than 180 days before `--as-of` (or before today without `--as-of`); the exit code stays 0. `CWLOGS_RETENTION_TODAY=YYYY-MM-DD` sets the day used as today.
- The monthly cost is labeled as an estimate. The column and the JSON key `usd_month` are now `estimated_usd_month` (rows and totals), the text output has one line `Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)`, and the JSON has `"estimate": true` and `"estimate_note"` at the top level.
- A closed standard output is noticed at start-up on Linux. Together with a failed write to standard output, including `--help` and `--version`, it ends with exit 2 and `cwlogs-retention: could not write output` on standard error. Standard output opened on `/dev/null` still exits 0. The detection is `src/stdout_probe.rs`, the only `unsafe` code; the library now forbids `unsafe`.
- `--fail-stored-bytes` accepts only a plain run of digits. A sign or a blank is a usage error.
- `--as-of` is checked before any file is read, so a bad date is reported first.
- A gzip file with bytes after its first member is an error. Those bytes were not read before.
- A file in a directory that is not JSON is skipped with a warning when it does not start with `{` or `[`. One that starts with `{` or `[` and does not parse is still an error.
- A file that holds only Unicode whitespace is "not JSON". Only space, tab, CR, and LF count as blank in a JSON file, in a numeric string, in a price file value, and at the end of a table line.
- A price with more than 28 fraction digits, or with more than 28 significant digits in all, is rejected. A rate such as `1000000000000000000.0049999999999` was rounded to a different rate before and gave a wrong amount.
- A newline, a carriage return, and every other escaped character are written as `\u{a}`, `\u{d}`, and so on in paths, names, warnings, errors, and usage errors, never as `\n`. This was `\n` and `\r` before. In a path or an argument that is not UTF-8 the invalid bytes are written as `\xNN` and the valid rest is escaped the same way; a newline in such a path used to be `\n`.
- With a closed standard output, a usage error or an input error is shown on standard error as for any other output (exit 2). Before, `cwlogs-retention --bogus >&-` and a missing file gave only `could not write output`. A run that would have printed a report, `--help`, and `--version` still end with `could not write output`.
- Amounts are exact. The monthly amount, each saving, and both totals are worked out in whole numbers (bytes, the rate mantissa, and the age in milliseconds as a fraction of a day) and rounded once to the cent, half away from zero. Before, rows and sums were decimals of 28 digits, so a total of three or more groups and a saving with an age that is not a whole number of days could come out one cent low when the exact amount was a half cent (24.76 for 24.765, 0.58 for 0.585). An amount that does not fit the decimal type after rounding is `price overflow` and no longer a cut amount. README "Money precision" and SECURITY.md say so.
- `.github/workflows/ci.yml`: `timeout-minutes: 30` for the test jobs, and `--no-fail-fast` for the test steps.
- The README text for C22 says that each differing ARN region is one warning, not one warning per run.
- `.gitattributes` keeps every text file on LF (`* text=auto eol=lf`), so the man page, the examples, and the byte-exact test data match in a checkout with CRLF endings.
- `age_days` shows at most 6 decimals, half away from zero, in text and JSON. The savings still use the full value.
- The table column width counts characters, not bytes.
- The advice line quotes the log group name: `--log-group-name='NAME'`.
- Raw file bytes are dropped after each file is parsed, not kept until all files are read.

### Fixed

- A total that is exactly a half cent over groups with many different ages made the run last minutes to hours (cubic in the number of groups); 1000 pairs of groups with a different age each took over a minute. The exact decision now adds rows with the same denominator into one numerator, reduces those, and adds them over their least common multiple, with a fixed limit of 4096 different denominators and 2048 bits. Within the limit, an exact half cent still rounds up once (0.125 is 0.13, and the monthly total, each saving, and the savings total are rounded once, half away from zero). Past the limit, the run ends with the error `total is too close to a half cent to round exactly within the work limit` (exit 2) at once, and no amount is guessed. README "Money precision" and SECURITY.md say so, with the measured times.
- `cargo test` in the unpacked `.crate` failed in `packaged_files_have_no_local_paths_internal_names_or_ai_names`: cargo writes the `exclude` list of the packaged `Cargo.toml` over three lines, and the check only let the line starting with `exclude` pass; without a git checkout, `.github/` was missing and the test panicked. Both cases are handled and tested.
- `directory_symlink_is_skipped` failed when the temporary directory path held the digits `512`, because the test looked for them in the output; it now looks at the table rows.
- A usage error with an argument that is not UTF-8 (`--` plus byte 0xff, a newline, and `FAKE: line`) printed the line after the newline as a line of its own. The argument is now escaped as for UTF-8 arguments.
- `--fail-stored-bytes` no longer takes a warning for a skipped member just because it starts with the word `skipped`: a file named `skipped a.json` with a `nextToken` is not a skipped member. The skipped members are kept in a list of their own (`Document::skipped`).
- A directory member that was skipped (nested symlink, non-regular file, a file that is not UTF-8, or a text note) with `--fail-stored-bytes` set is an error with exit 2, because the total would be missing that member.
- The test code passes `cargo clippy -D warnings` on Rust 1.85 (a spawned child is waited for on every path) and on Windows and macOS (a helper used only by a Linux test is Linux only). Tests that need a file name that is not UTF-8, an escape character in a name, `/dev/full`, or a closed descriptor run on Linux only. The FIFO test no longer needs an external `timeout` command.
- The price source is shown with control, invisible, line-breaking, and direction-changing characters escaped, in text and in JSON.
- The set of escaped characters in paths, names, warnings, and errors now also covers soft hyphen, fillers, variation selectors, isolates, directional marks, the word joiner range, the byte order mark, interlinear annotation marks, and tag characters.
- A usage error from the argument parser shows control and direction characters of the given arguments escaped.
- A path that is not UTF-8 is shown as escaped bytes on Unix.


### Added

- GitHub Actions CI on four systems, a Rust 1.85 job, `cargo audit`, and `cargo deny` for licenses.
- `deny.toml` with a permissive-only license list.
- `QUALITY.md` with the test count.

### Changed

- Tests that need Unix paths, symlinks, FIFOs, or control characters in file names run only on Unix.
- `.gitattributes` keeps `*.md` on LF endings.

This version is not published and has no git tag.

## [0.1.0] - 2026-11-09

### Added

- `cwlogs-retention` reads local `aws logs describe-log-groups` JSON and prices archived storage per log group.
- Groups with no retention, and a steady-ingest savings figure at 30, 90, and 365 days. The report says this is not a forecast.
- Built-in storage rates from the AWS CloudWatch pricing page, read on 2026-11-09, and a `--prices` file for another quote.
- Text table and `--json`. `--fail-stored-bytes` exits 1 when an unretained group is above the threshold. The retention advice is text and is not executed.

This version is not published and has no git tag.
