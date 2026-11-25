# Examples

`describe-log-groups.json` is an invented page of `aws logs describe-log-groups`: three log groups, no real account, organisation, or token. The account id in the ARNs is the documentation placeholder `123456789012`. Run every command from the repository root; none of them writes a file.

## Text report

```sh
cwlogs-retentie --as-of 2026-11-09 examples/describe-log-groups.json
```

Expected output: `examples/describe-log-groups.expected.txt`

Expected exit code: 0

`--as-of` fixes the date the ages are measured to, so the output is the same on every day. Without it the savings columns are left out. The price data is dated 2026-11-09; on a later day the run adds a `warning: price data dated ...` line on standard error once it is older than 180 days, and the exit code stays 0. The tests set `CWLOGS_RETENTIE_TODAY=2026-11-20` so that no warning appears.

## Markdown and JSON

The same file with `--md` and with `--json` is shown in the `README.md` section "Example"; `tests/readme.rs` runs those commands and compares the output. `tests/readme.rs` also runs the command above and compares it to `examples/describe-log-groups.expected.txt`.
