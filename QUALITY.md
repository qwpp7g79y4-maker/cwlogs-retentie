# Quality

This commit changes documentation only; the measurements below apply to code commit 829cc6e.

Suite: 229 passed, 0 ignored, 0 failed.

Record of 2026-11-25, run on a clean worktree of commit 829cc6e64e1d1d8d90820d5ef77b7b8757cc359d (`829cc6e`), the last commit that changed code or tests. Later commits change documentation only. rustc 1.99.0, cargo 1.99.0, Linux x86_64 (glibc). Every result in the table below comes from this one run on one local machine. Version 1.0.0 is not published and has no git tag.

Only the four commands below were run in this measurement. Anything else in this file that is not in that table (per-target details beyond the counts, red checks, time and memory figures, closed standard output, known limits) is an earlier record and was not repeated here.

CI (in `.github/workflows/ci.yml`) was not read or started in this measurement: CI result not run here.

## Commands

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo test --locked` (229 passed) | 0 |
| `cargo test --release --locked` (229 passed) | 0 |

Record of 2026-11-25, Rust 1.85.1 (rustc 1.85.1, cargo 1.85.1), run on the same code commit 829cc6e (commit df6514c has the same code), Linux x86_64 (glibc), one local machine:

| Command | Exit |
| --- | ---: |
| `cargo +1.85 fmt --check` | 0 |
| `cargo +1.85 clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo +1.85 test --locked` (229 passed) | 0 |
| `cargo +1.85 test --release --locked` (229 passed, 0 failed, 0 ignored, 2026-11-25) | 0 |

Everything else is not run in this measurement: `cargo package`, `cargo deny`, `cargo audit`, `cargo about`, the unpacked `.crate`, fuzzing, mutation testing, Windows, macOS, musl, and GitHub Actions.

## Test result per target

`cargo test --locked` in the debug profile and `cargo test --release --locked` give the same counts.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| unittests `src/lib.rs` | 72 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 |
| `tests/claims.rs` | 27 | 0 | 0 |
| `tests/cli.rs` | 106 | 0 | 0 |
| `tests/docs_clean.rs` | 8 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 |
| `tests/readme.rs` | 15 | 0 | 0 |
| doc-tests | 0 | 0 | 0 |
| total | 229 | 0 | 0 |

Debug and release were run as listed under "Commands"; both exit codes are 0.

Red checks. A test that records a change is run before the change and shown red, then green after:

- `cargo test --offline` in the unpacked `.crate` of commit 0d5ee26 (made with `cargo package`, 35 files, no git history): `packaged_files_have_no_local_paths_internal_names_or_ai_names` was red on lines 20 and 21 of the packaged `Cargo.toml` (cargo wrote `exclude` over three lines). Green after the change: all tests pass in the unpacked `.crate` (175 at that commit, 185 after the money change below). A packaged copy without `.github/` is handled by `a_missing_directory_is_skipped_only_where_it_is_optional`.
- `directory_symlink_is_skipped`: red with the temporary directory set to a path that holds `512` (`TMPDIR=<path with 512>`, panic at `tests/cli.rs:477`), green after the change, with and without that path.
- The tests `total_of_stored_bytes_over_u64_is_an_input_error_without_a_name`, `a_digit_string_is_at_most_20_characters_also_with_leading_zeros`, `each_differing_arn_region_is_one_warning`, and `money_precision_limits_are_as_documented` record behavior of the program that was measured before and not changed; they were green before and are green after, and a change of that behavior turns them red. `money_precision_limits_are_as_documented` is the exception: the money change below made it red on purpose, and it now pins exact amounts and the `price overflow` case.
- Money change (exact amounts, rounded once): `a_total_that_is_an_exact_half_cent_rounds_up`, `a_saving_that_is_an_exact_half_cent_rounds_up`, `a_savings_total_is_rounded_once_from_the_exact_rows`, `totals_of_odd_multiples_of_2_pow_29_match_the_integer_oracle`, and `savings_at_an_exact_half_cent_with_a_retention_round_up` were red on commit 1774c27 (24.76 instead of 24.77, 0.58 instead of 0.59, 401.63 instead of 401.64, 32.26 instead of 32.27) and are green after. `savings_match_the_integer_oracle_for_ages_retentions_and_horizons` and `every_class_rate_is_exact_in_rows_and_totals` were green on that commit too (no failing case was found for them) and are regression tests only.
- Sum of many ages (exact half cent, rounded once): `a_half_cent_sum_of_many_denominators_rounds_once` and `distinct_ages_on_an_exact_half_cent_round_once_per_horizon` were red on commit 673cce2 (`price overflow`, exit 2, on the unit test and on the 31-day span of the CLI test) and are green after. The unit test also pins the amount just under a half cent and a 28-digit rate just under a half cent, both summed with the same wide denominators, at 547.00. The CLI test pins the monthly total and the saving at 30, 90, and 365 days.
- Work limit of the exact sum: `interleaved_pairs_on_an_exact_half_cent_do_not_hang` (1000 pairs of groups, the two groups of a pair 1000 names apart, a different age on every pair, a 30-day saving of exactly 0.045) was red on commit 142508d: the run was killed after 20 s with no output (the review reports measured 73 s for the same shape). Green after: 0.04 s, monthly total 72.46 and saving 0.05, equal to Python `Fraction` arithmetic (7246 and 5 cents; also 14488 and 28972 cents for 2000 and 4000 pairs). The unit tests `many_different_denominators_near_a_half_cent_are_refused_at_once`, `a_common_denominator_past_the_limit_is_refused`, `rows_with_one_denominator_add_into_one_numerator`, and `a_few_different_denominators_near_a_half_cent_still_round_exactly` name `MAX_DENOMINATORS` and the `terms` field, which did not exist on commit 142508d, so they did not compile there. Worst case of the exact step inside the limits, measured once in release with 4000 different denominators whose common multiple stays within 2048 bits: 0.35 s.

Red checks on a scratch copy outside the repository (kept outside, not part of the package, removed afterwards):

- `tests/handbook_help.rs` failed (`tests/handbook_help.rs:81`, the OPTIONS section of the man page must describe exactly the long options of `--help`) when the `--json` entry was removed from the OPTIONS section of `man/cwlogs-retentie.1`, and failed (`tests/handbook_help.rs:61`, the man page must contain the `--help` text verbatim) when one word of the option text was changed there.
- `tests/docs_clean.rs` failed, with the file and line, on one injected text each in `SECURITY.md`, `README.md`, `CHANGELOG.md`, `QUALITY.md`, the man page, `tests/cli.rs`, `.gitignore`, `Cargo.toml`, and `CHANGELOG.md` again: a local absolute path (two kinds), a home-relative path, an option of an internal wrapper (two), both internal file names, an internal tool name (two), and in `.gitignore` a comment that names a file. It failed in `Cargo.toml` when either entry was missing from `exclude`, as one line and as the block cargo writes.

## Not run

- Not run in this measurement: `cargo package` and the unpacked-`.crate` test run, `cargo deny`, `cargo audit`, `cargo about`.
- A fuzz campaign.
- A mutation run.
- GitHub Actions. The workflows `ci.yml` and `release.yml` were not started (the CI account has a payment problem), so there is no CI result on any system and no release archive.
- Windows and macOS. No test was run on either system, and the `cargo check` and clippy runs for `x86_64-pc-windows-gnu` and `x86_64-apple-darwin` of the 2026-11-19 record were not repeated for this record (`build.rs` and the `--version` constant are new since then). Tests that need Unix paths, symlinks, FIFOs, or control characters in file names run only on Unix. Tests that need `/dev/full`, a closed descriptor, a file name that is not UTF-8, or an escape character in a file name run only on Linux.
- The test suite on musl (see "Known limits").
- `mandoc -Tlint` on the man page (not installed); `man -l man/cwlogs-retentie.1` renders without a message.
- `git tag` and `cargo publish`; nothing is published.
- The package job of the workflow. Its steps were run by hand as listed under "Commands".

## Closed and full standard output

`src/stdout_probe.rs` and `src/interrupt.rs` are the only `unsafe` code, and they are in the binary only; the library forbids `unsafe`. Rust replaces a closed file descriptor 1 with `/dev/null` before `main` runs, so afterwards a closed standard output looks like `1<>/dev/null`. The probe runs a constructor before that replacement and records whether descriptor 1 was already closed, using a read-only `fcntl(1, F_GETFD)`. If the run would print anything to standard output, `main` exits 2 with `could not write output`; a usage error or an input error is shown on standard error first, as without the redirection. A failed write of the report, of `--help` or of `--version` ends the same way. The probe works on Linux only and does not read `/proc`.

Measured by hand in an earlier record, before the `--output`, SIGINT, and `build.rs` commits, and not repeated for this record; the closed and full standard output tests of `tests/cli.rs` ran green in this record. The runs used a release binary and `examples/describe-log-groups.json`; every row is a separate run of the binary. The debug binary gave the same exit codes and messages:

| Run | Exit | Message |
| --- | ---: | --- |
| `cwlogs-retentie examples/describe-log-groups.json >&-` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie examples/describe-log-groups.json >/dev/full` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie examples/describe-log-groups.json >/dev/null` | 0 | none |
| `cwlogs-retentie examples/describe-log-groups.json 1<>/dev/null` | 0 | none |
| `python3 -I`, `subprocess.run(..., stdout=subprocess.DEVNULL)` | 0 | empty |
| `cwlogs-retentie examples/describe-log-groups.json \| true` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --help >&-` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --help >/dev/full` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --help >/dev/null`, `--help 1<>/dev/null` | 0 | none |
| `cwlogs-retentie --help \| true` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --version >&-` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --version >/dev/full` | 2 | `cwlogs-retentie: could not write output` |
| `cwlogs-retentie --version >/dev/null`, `--version 1<>/dev/null` | 0 | none |
| `cwlogs-retentie --version \| true` | 2 | `cwlogs-retentie: could not write output` |
| `python3 -I`, `stdout=subprocess.DEVNULL` for `--help` and `--version` | 0 | empty |
| `cwlogs-retentie --bogus >&-` | 2 | `error: unexpected argument '--bogus' found` and the usage |
| `cwlogs-retentie /nonexistent/x.json >&-` | 2 | `cwlogs-retentie: /nonexistent/x.json: No such file or directory (os error 2)` |
| Report, `--help`, `--version` with `/proc` replaced by an empty tmpfs (`unshare -Urm`), standard output to a file | 0 | none; 1491, 854 and 22 bytes written |

No run panicked. The `| true` case with the report exited 2 in 40 of 40 repeats; it depends on `true` closing the pipe before the report is written. The tests in the `stdout_closed` module of `tests/cli.rs` cover a closed and a full standard output for the report, `--help` and `--version`, `/dev/null` and `1<>/dev/null`, a null `Stdio` handle, and a broken pipe that is forced by closing the read end before any input is sent.

## Known limits

- This page records checks. It is not a guarantee and it is not a fuzz report.
- Closed-stdout detection (`src/stdout_probe.rs`) works on Linux only. On other systems a closed standard output is not told apart from `/dev/null`.
- A failed write of a warning to standard error is ignored: `cwlogs-retentie --json DIR 2>/dev/full` with a skipped member exits 0 (measured: `--json one.json 2>/dev/full` exits 0).
- The built-in rates are the us-east-1 examples of the pricing page, read on 2026-11-09, and are not checked against the live page.
- `nextToken` is not followed, so one response page is not the whole account.
- The 268435456-byte limit applies to the unpacked size of a file or of standard input.
- Time and memory at that limit (release build, one Linux machine): a 268434299-byte file with 1348916 groups took 10.5 seconds and 2.7 GB at its peak; a file one byte over the limit is refused at once in 2.7 MB.
- Memory is not bounded by the file size, because every field of the file is held in memory (`serde_json::Value`). Measured with the release binary, one group and an unknown field with a list of numbers: 10000059 bytes took 0.7 s and 0.53 GB, 40000059 bytes 2.7 s and 2.1 GB, 100000059 bytes 6.9 s and 5.3 GB (about 53 times the file size). One million groups (69777795 bytes) took 3.4 s and 0.89 GB as text and 9.0 s and 4.6 GB with `--json`. Reading without a tree for unknown fields, or writing `--json` as a stream, would lower this; it is not done.
- A gzip file inside a gzip file is `not UTF-8`, not a gzip error. The error `creationTime is after --as-of` names the log group and not the file. Both are kept as they are.
- `retentionInDays` as a string of digits is accepted (`"14"` and `"0014"` are 14); this is stated in the README.
- Money precision (README, "Money precision"): amounts are exact and rounded once to the cent, so a monthly amount, a total, a saving, and a savings total that are exactly a half cent round up, including a sum of groups with different ages. An amount that does not fit the decimal type after rounding (rate `1000000000000000000`, 18446744073709551615 bytes) is `price overflow` and no longer a cut amount. A total on a half cent over more than 4096 different denominators, or a common denominator over 2048 bits, is the error `total is too close to a half cent to round exactly within the work limit` (exit 2). Time and memory for the text table with `--as-of 2026-11-09` on 100000, 200000, and 400000 groups with a different age each (release build, the shorter of two runs, the higher peak): 1.71 s and 152876 KB, 3.53 s and 305268 KB, 6.85 s and 566644 KB. From the 100000-group run, time grew by 2.06 and 4.00, and peak memory by 2.00 and 3.71. With an exact half cent over 1000, 2000, and 4000 pairs of groups: 0.04 s, 0.08 s, 0.17 s (grew by 2.0 and 4.3) and 17020, 30724, 58712 KB (grew by 1.8 and 3.4).
- The total of the stored bytes must fit in 18446744073709551615; two groups of that size are `stored bytes overflow` (exit 2, no file or group in the message). A number written as a string is at most 20 characters; `"018446744073709551615"` and a `retentionInDays` string of 21 characters are `bad`. Both fail closed.
- A price has no upper limit. A rate of `1000000000` is accepted and printed; `79228162514264337593543950335` with 1 GiB prints `79228162514264337593543950335.00`, and with `18446744073709551615` bytes it is `price overflow`. A limit would be a product decision.
- Valid JSON without log groups (`42`) inside a directory stops the run with exit 2, while a text note is skipped with a warning. Both fail closed when `--fail-stored-bytes` is set; without it the two differ. This is kept as it is.
- With `--json`, the price `source` is the escaped text, so a source with U+202E is `x\\u{202e}y` in the document and not the original characters.
- A musl build (`x86_64-unknown-linux-musl`, release) compiled and gave exit 2 with the message for `>&-` and `>/dev/full`; the test suite was not run on it. The release binary built on this machine (glibc 2.35) needs glibc 2.34 at most (`objdump -T`); no older system was tried.
- "No network call" is checked through the dependency list (`cargo tree -e normal`: `clap`, `flate2`, `rust_decimal`, `serde_json`) and the source, which has no socket, HTTP, or process code. No test watches the system calls.

## Open points

- Money formula: no open point. The amounts are exact; the test `money_precision_limits_are_as_documented` and the README section "Money precision" change together with any change of the formula.
- There is no glibc check script and no fuzz directory. Nothing is published. What was not run is listed once, in "Not run".
- The `homepage` field in `Cargo.toml` names the author's website. It has to be confirmed before a release.
- The claim table below is checked only for form (`quality_claims_table_names_a_test_for_every_claim`): each of C1 to C24 has a row whose first test exists. That the test proves the whole claim is read by hand.

## Claim → test

Every claim of `README.md`, with the test that proves it. Test names are from the 2026-11-20 run (test counts: 227). "Gap" means no test proves it, or only part of it.

| # | Claim | Test | Status |
| --- | --- | --- | --- |
| C1 | One object is one group; envelope, bare array, single object | `claim_c1_one_object_is_one_group`, `envelope_array_and_single_object` | tested |
| C2 | Storage USD is bytes / 1024^3 × rate, computed exactly and rounded once half away from zero, total from the exact amounts, 1,024 GB is 1 TB, 30 TB is 921.60 | `claim_c2_gibibyte_and_terabyte`, `page_thirty_terabytes_is_921_60`, `usd_rounds_half_away_from_zero_to_the_cent`, `many_small_groups_total_is_not_the_sum_of_rounded_rows`, `directory_total_is_the_sum_of_the_single_files`, `a_half_cent_from_a_30_digit_gb_rounds_up_like_the_exact_amount`, `a_total_that_is_an_exact_half_cent_rounds_up`, `a_saving_that_is_an_exact_half_cent_rounds_up`, `savings_match_the_integer_oracle_for_ages_retentions_and_horizons`, `a_half_cent_sum_of_many_denominators_rounds_once`, `distinct_ages_on_an_exact_half_cent_round_once_per_horizon` | tested; the overflow limit is pinned by `money_precision_limits_are_as_documented` (see L22) |
| C3 | Standard rate 0.03 | `claim_c3_standard_rate` | tested |
| C4 | Infrequent Access 0.03 in the example, rate per class from a price file | `claim_c4_infrequent_access_rate_and_override`, `prices_override_uses_the_class_rate` | tested |
| C5 | Delivery rate 0.03 | `claim_c5_delivery_rate` | tested |
| C6 | Source, date, and region are printed | `claim_c6_source_date_region`, `builtin_matches_the_pricing_page` | tested |
| C7 | The 5 GB free tier is not subtracted | `claim_c7_free_tier_not_subtracted`, `six_gibibytes_do_not_use_the_five_gb_waiver` | tested |
| C8 | Ingestion and other items are not priced | `claim_c8_ingestion_not_priced` | tested |
| C9 | Missing or null `retentionInDays` never expires | `claim_c9_missing_retention_never_expires`, `retention_rules` | tested |
| C10 | Steady-ingest horizon and saved USD | `claim_c10_steady_ingest_horizon`, `horizon_is_the_shorter_of_age_and_retention`, `zero_horizon_with_bytes_has_no_savings` | tested |
| C11 | Targets 30, 90, 365; at or above the horizon saves 0 | `claim_c11_targets_and_zero_at_horizon` | tested |
| C12 | `--as-of` is midnight UTC, age in days, `age_days` at most 6 decimals | `claim_c12_as_of_midnight_utc`, `half_day_age_is_exact`, `fractional_age_shows_at_most_six_decimals`, `as_of_accepts_only_digits`, `invalid_as_of_is_rejected` | tested |
| C13 | Without `--as-of` or `creationTime` savings are omitted | `claim_c13_savings_omitted_without_as_of_or_creation`, `savings_need_an_as_of_date_and_a_creation_time` | tested |
| C14 | `--fail-stored-bytes` exits 1 for an unretained group; report still printed; skipped member (symlink, non-regular file, not UTF-8, text note) is exit 2 | `claim_c14_fail_stored_bytes_exit_1`, `threshold_ignores_retained_groups_and_equal_bytes`, `exit_1_still_prints_the_report`, `fail_stored_bytes_refuses_when_a_member_was_skipped`, `fail_stored_bytes_refuses_when_a_non_utf8_member_was_skipped`, `a_file_name_that_starts_with_skipped_is_not_a_skipped_member` | tested (a skipped symlink or FIFO has its own Unix test: `directory_symlink_is_skipped`, `directory_skips_a_fifo`) |
| C15 | `--json` has money strings, source, date, final newline | `claim_c15_json_money_strings`, `console_examples_match_the_binary` | tested |
| C15a | The monthly cost is labeled an estimate: column and JSON key `estimated_usd_month`, one `Estimate from list prices` line in the text, `"estimate": true` in the JSON | `text_output_labels_the_monthly_cost_as_an_estimate`, `json_output_labels_the_monthly_cost_as_an_estimate`, `console_examples_match_the_binary` | tested |
| C15b | Price data older than 180 days at `--as-of` (or today) warns on standard error, exit stays 0; exactly 180 days does not warn | `builtin_prices_older_than_180_days_at_as_of_warn_on_stderr`, `prices_file_older_than_180_days_warns_with_json_and_keeps_exit_0`, `prices_file_within_180_days_does_not_warn_and_a_future_date_does_not_warn`, `without_as_of_the_price_age_is_measured_against_today`, `price_age_uses_as_of_or_the_passed_today_and_never_the_clock` | tested |
| C15c | The documented `describe-log-groups` shape (`tests/fixtures/describe-log-groups-documented.json`: every documented field, minimal entries, empty `inheritedProperties`, both `arn` and `logGroupArn`, names with `_`, `.`, `#`, `/`, a page with `nextToken`, an empty page) is read without error and priced exactly (4.05 USD for 135 GB) | `documented_shape_with_arn_and_log_group_arn_prices_exactly`, `documented_shape_text_output_has_every_name_and_the_same_total`, `documented_pages_with_a_next_token_an_empty_page_and_a_directory_are_read`, `ambiguous_region_and_shape_are_rejected` | tested; written from the documentation without network (https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_DescribeLogGroups.html and https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_LogGroup.html), so a field added to the API later is not covered |
| C15d | `--quiet` prints only the estimate line, the table, and threshold lines, hides warnings, keeps JSON whole and the exit codes; `--verbose` adds `progress:` lines on standard error only; both together exit 3 | `quiet_together_with_verbose_is_a_usage_error_with_exit_3`, `quiet_prints_only_the_result_and_hides_warnings`, `quiet_keeps_json_whole_and_the_threshold_exit_code_and_still_reports_errors`, `verbose_adds_progress_on_stderr_and_leaves_stdout_alone`, `help_lists_quiet_and_verbose` | tested |
| C15e | `--md` prints a Markdown table with the amounts of the text and the JSON, one estimate line, warnings as quoted lines, `|` and line breaks escaped in cells; with `--json` it exits 3 | `markdown_table_has_the_amounts_of_the_text_and_the_json`, `markdown_keeps_warnings_quiet_and_json_conflict_rules`, `a_cell_cannot_break_the_table`, `console_examples_match_the_binary` | tested |
| C15f | SIGINT (Unix) ends the run with exit 130, the line `cwlogs-retentie: interrupted by SIGINT` on standard error, and an empty standard output, in text, `--json`, and `--md`; a report that is already being written is finished first | `sigint::sigint_exits_130_with_one_line_and_no_output` | tested on Linux with a real `kill -INT`; not tried on macOS; the write-in-progress case is by construction, not by a test |
| C15g | `--output FILE` writes the whole report through a temporary file in the same directory and a rename; a symlink is replaced, never followed; a missing directory, a directory in the way, or a failed write is exit 2 with a clean message and no temporary file; a failed run leaves the old file | `output_writes_the_report_to_a_file_and_replaces_an_existing_one_atomically`, `output_keeps_the_threshold_exit_code_and_still_writes_the_file`, `output_is_not_touched_when_the_run_fails`, `output_write_errors_exit_2_with_a_clean_message`, `output_does_not_follow_a_symlink_and_replaces_the_link`, `help_lists_output` | tested on Linux; a full disk was not tried; an interrupted rename is not tested, a SIGINT during the write is held until the rename is done |
| C15h | Every JSON document starts with `"schema_version": 1`, also with `--quiet`, `--verbose`, `--output`, a directory, standard input, and exit 1; a failed run prints no JSON | `every_json_output_path_has_schema_version_one`, `json_error_paths_print_no_json_document_and_no_schema_version`, `console_examples_match_the_binary` | tested; the second test also passes without the change, because no error path ever printed JSON |
| C16 | The advice line is text, names `put-retention-policy`, says not executed | `claim_c16_advice_is_not_executed` | tested |
| C17 | No AWS SDK, no network, `nextToken` not followed or printed | `claim_c17_no_sdk_and_next_token_not_followed`, `next_token_is_not_stored` | tested in part: dependency list and source only, no network test |
| C18 | Duplicate names are an error without the raw JSON | `claim_c18_duplicate_names`, `duplicate_name_does_not_include_the_body` | tested |
| C19 | `storedBytes` is a non-negative integer, bad values not echoed | `claim_c19_stored_bytes_integer`, `bad_name_and_bad_stored_bytes_are_not_echoed`, `large_integer_is_not_rounded` | tested |
| C20 | Name is 1 to 512 characters of the alphabet, bad name not echoed | `claim_c20_log_group_name_alphabet`, `name_alphabet_bounds` | tested |
| C21 | Gzip by magic bytes, bytes after the first member are an error | `claim_c21_gzip_is_by_magic_bytes`, `gzip_bytes_after_the_first_member_are_an_error`, `corrupt_gzip_is_an_error` | tested |
| C22 | Account ids not printed, region mismatch is one warning | `claim_c22_arn_account_hidden`, `bad_class_is_not_echoed_and_arn_keeps_only_the_region`, `each_differing_arn_region_is_one_warning` | tested |
| C23 | Rounded rows do not add up to the total; no per-day extrapolation | `claim_c23_total_is_the_sum`, `many_small_groups_total_is_not_the_sum_of_rounded_rows` | tested in part: the absence of per-day figures is not tested |
| C24 | Errors hide contents; control and direction characters are escaped | `claim_c24_errors_hide_file_contents`, `control_chars_in_a_path_stay_on_one_line`, `escape_and_bell_in_a_path_stay_on_one_line`, `line_separator_and_bidi_override_are_not_raw`, `isolates_and_marks_are_escaped_in_labels_and_warnings`, `added_invisible_characters_are_escaped`, `bidi_and_line_separators_in_source_are_not_raw_in_output`, `clap_usage_errors_escape_raw_user_input`, `clap_errors_show_newline_cr_and_escape_as_unicode_escapes_on_one_line`, `symlink_warning_escapes_the_path`, `clap_error_with_a_non_utf8_argument_stays_on_one_line`, `newline_in_a_non_utf8_path_is_shown_as_a_unicode_escape`, `non_utf8_file_name_is_shown_as_escaped_bytes` | tested (Linux for the non-UTF-8 cases) |
| L1 | Install from a local path, Rust 1.85; the release workflow is named | `installation_uses_a_local_path` (`tests/readme.rs`) | tested (the text only; `cargo install` is not run) |
| L2 | The quick start reads `examples/describe-log-groups.json` and writes no file, in both READMEs | `quick_start_reads_the_sample_and_writes_nothing` | tested |
| L3 | The console examples are what the binary prints | `console_examples_match_the_binary`, `example_file_prices_the_three_groups`, `the_examples_folder_documents_each_command_and_its_expected_output` | tested |
| L4 | README sections follow the documentation standard in this order, the boundaries come before Installation, the usage block is the `--help` text, `README.nl.md` has the same console blocks and claims, both READMEs end with Contact, claims C1 to C24 each have a test, limitations match tests | `readme_sections_follow_the_documentation_standard`, `the_boundaries_come_before_installation`, `usage_block_is_the_help_text`, `the_dutch_readme_has_the_same_content`, `both_readmes_end_with_the_contact_section`, `every_how_it_counts_claim_has_a_test`, `limitations_name_behaviors_the_tests_cover` | tested |
| V1 | A debug build prints the commit after the version in `--version` and `-V`; a release build prints the version alone | `version_has_the_commit_in_debug_builds_only` | tested (debug and release runs) |
| Q1 | `QUALITY.md` and `QUALITY.nl.md` have a row with a real test for each claim C1 to C24, a date, a commit, and a list of what was not run | `quality_claims_table_names_a_test_for_every_claim`, `quality_names_the_tested_commit_the_date_and_what_was_not_run` | tested (the form only, not that the listed results are true) |
| L5 | A file over 268435456 bytes is rejected; stdin too | `file_larger_than_268435456_bytes`, `stdin_larger_than_268435456_bytes_is_an_error`, `gzip_that_expands_past_the_limit_is_an_error` | tested |
| L6 | A nested symlink is skipped with a warning, an explicit symlink is followed | `directory_symlink_is_skipped`, `explicit_symlink_is_followed` | tested (Unix) |
| L7 | A directory nested past 64 levels is an error | `directory_at_64_levels_is_accepted`, `directory_deeper_than_64_levels_is_an_error` | tested |
| L8 | With `--json`, warnings go to standard error | `json_warnings_go_to_stderr_and_hide_the_token` | tested |
| L9 | The savings figure is not a forecast; a shorter retention deletes logs | `limitations_name_behaviors_the_tests_cover` (checks the README words only) | gap: wording only |
| L10 | The built-in rates are the 2026-11-09 us-east-1 examples | `builtin_matches_the_pricing_page` | tested against the bundled price file, not against the live page |
| L11 | Not published, no git tag | `licenses_and_changelog_match_the_package` (the changelog says so only) | gap: the repository state is not tested |
| L12 | The test total and the counts per group in README and this file | `test_counts_in_the_readme_and_quality_match_the_sources` | tested (counts `#[test]` in the sources; passed counts per target are from the run above) |
| L13 | Exit code 0 ok, 1 threshold, 2 input or usage error | `help_and_version_exit_0`, `usage_errors_exit_2`, `missing_file_exits_2`, `claim_c14_fail_stored_bytes_exit_1` | tested |
| L14 | MIT OR Apache-2.0 | `licenses_and_changelog_match_the_package` | tested |
| L15 | Closed, full or broken standard output exits 2 with `could not write output`; `/dev/null` keeps exit 0 | `closed_stdout_is_exit_2_for_a_report`, `closed_stdout_is_exit_2_for_help_and_version`, `full_stdout_is_exit_2_for_report_help_and_version`, `full_stdout_via_a_file_handle_is_exit_2_for_help`, `dev_null_keeps_the_real_exit_code`, `null_stdio_handle_keeps_exit_0`, `broken_pipe_is_exit_2_without_a_panic`, `unwritable_stdout_exits_2`, `closed_stdout_still_shows_a_usage_or_input_error` | tested (Linux only; the warning to a full standard error is a known limit) |
| L16 | Input rules: 127 levels of nesting are read and 128 are not JSON; `storedBytes` string with ASCII blanks only; retention 1 to 3653 as a number or a string of digits, other CloudWatch values warn; BOM read, UTF-16 is not UTF-8 | `json_nested_127_levels_is_read_and_128_is_not_json`, `stored_bytes_string_with_unicode_space_is_an_error`, `retention_rules`, `retention_as_a_string_of_digits_is_read_like_the_number`, `bom_is_accepted`, `strict_not_utf8_is_an_error_without_the_bytes`, `a_digit_string_is_at_most_20_characters_also_with_leading_zeros`, `total_of_stored_bytes_over_u64_is_an_input_error_without_a_name` | tested |
| L17 | A price is a plain decimal of at most 40 characters, 28 decimals and 28 significant digits; no upper limit | `rate_with_more_than_28_significant_digits_is_rejected_not_rounded`, `money_must_round_trip_digit_for_digit`, `money_rejects_exponent_sign_and_empty`, `huge_rate_is_an_input_error` | tested in part: the absence of an upper limit is measured by hand (see "Known limits"), not by a test |
| L18 | Directory members: text, non-UTF-8 and empty members are skipped with a warning; valid JSON without log groups is exit 2 | `directory_skips_a_text_note_with_a_warning`, `directory_skips_non_utf8_and_empty_members`, `directory_member_with_json_but_no_log_groups_stops_the_run` | tested |
| L19 | With `--json` the price `source` is the escaped text | `json_output_shows_the_price_source_as_escaped_text` | tested |
| L20 | 10 seconds and 2.7 GB at the size limit; memory about 53 times the file size for unknown fields, `--json` 4.6 GB for a million groups; glibc 2.34; builds and runs on glibc and musl; compiles for Windows and macOS | none | gap: measured by hand (see "Known limits" and "Not run"), no test |
| L21 | A gzip file in a gzip file is `not UTF-8`; the `creationTime` error names the group, not the file | none | gap: measured by hand with the binary, no test |
| D1 | No local path, internal tool or folder name, option or file name, or AI system name in README, QUALITY, CHANGELOG, SECURITY, the licenses, the man page, `Cargo.toml` (except the `exclude` list), `deny.toml`, `.gitignore` (except its two entries), `.gitattributes`, `.github/`, `src/`, `examples/`, `prices/`, or `tests/`; the check also passes in the unpacked `.crate` and fails when `exclude` loses an entry | `packaged_files_have_no_local_paths_internal_names_or_ai_names`, `cargo_toml_keeps_both_ignored_files_out_of_the_package`, `the_exclude_list_may_be_one_line_or_the_block_cargo_writes`, `a_path_missing_from_exclude_is_found`, `only_the_two_entries_may_stand_in_gitignore`, `a_missing_directory_is_skipped_only_where_it_is_optional`, `the_check_itself_finds_each_pattern_and_passes_plain_text`, `this_source_does_not_hold_the_names_it_looks_for` | tested; the run in the unpacked `.crate` is a CI step (`package`) and was run by hand |
| S1 | `SECURITY.md` names what to report: a file name, log group name, or price field that adds a line or an escape; contents echoed in an error; writing, running, or connecting; reading past the size limit; an amount that changes without an error | `claim_c24_errors_hide_file_contents`, `clap_errors_show_newline_cr_and_escape_as_unicode_escapes_on_one_line`, `line_separator_and_bidi_override_are_not_raw`, `file_larger_than_268435456_bytes`, `claim_c17_no_sdk_and_next_token_not_followed` | tested in part: the **Security** tab, the e-mail address, and the 7-day answer are process claims without a test |
| M1 | The man page holds the `--help` text and the same long options | `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` (`tests/handbook_help.rs`: the usage text verbatim, the same long options, each option in the OPTIONS section) | tested; the other man page text is not compared with the code |
| L22 | Money precision: the cases of the README section "Money precision" | `money_precision_limits_are_as_documented`, `amounts_that_do_not_fit_are_an_error_not_a_cut_amount`, `a_half_cent_sum_of_many_denominators_rounds_once`, `distinct_ages_on_an_exact_half_cent_round_once_per_horizon`, `interleaved_pairs_on_an_exact_half_cent_do_not_hang`, `many_different_denominators_near_a_half_cent_are_refused_at_once`, `a_common_denominator_past_the_limit_is_refused` | tested (exact amounts, the half-cent sum over many ages, the work limit, and the overflow case are pinned; the random comparisons use whole-number arithmetic in the tests) |

L11, L20, and L21 are gaps, L9 is a wording check, and C17, C23, L17, and S1 are tested in part. Every other row has a test.
