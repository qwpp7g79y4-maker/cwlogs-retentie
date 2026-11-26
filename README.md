# cwlogs-retention

[![crates.io](https://img.shields.io/crates/v/cwlogs-retention.svg)](https://crates.io/crates/cwlogs-retention) [![downloads](https://img.shields.io/crates/d/cwlogs-retention.svg)](https://crates.io/crates/cwlogs-retention) [![CI](https://github.com/qwpp7g79y4-maker/cwlogs-retention/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retention/actions/workflows/ci.yml) [![PEPAI checked](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fqwpp7g79y4-maker%2Fcwlogs-retention%2Fmain%2F.github%2Fbadge.json)](QUALITY.md) [![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retention/blob/main/Cargo.toml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retention#license)

Estimate what the stored bytes of your CloudWatch Logs log groups cost per month, find the groups that never expire, and see what a 30, 90, or 365 day retention would save. It is for anyone with an `aws logs describe-log-groups` export who wants the numbers without sending the export anywhere. The result is a table of USD amounts that you can check by hand (see "How it counts").

The tests and the measured numbers are in `QUALITY.md`.

A `describe-log-groups` export already has the stored bytes and the retention of each log group. cwlogs-retention prices that archived storage on your machine and says which groups never expire. It does not call AWS and it does not need an account.

What it does and does not do:

- It reads the files you name (JSON, gzip, a directory, or standard input) and the price file if you pass one. It uses no network: there is no AWS SDK and no `nextToken` is followed.
- It writes nothing, except the one file you name with `--output`. It never runs a command: the advice lines show `aws logs put-retention-policy ...` and say they were not executed.
- It needs no account, token, or paid service.
- Every amount is an estimate from list prices, not your AWS invoice; the output says so. The built-in prices are dated 2026-11-09 and the tool warns when price data is more than 180 days old.

## Installation

With Rust 1.85 or newer, from a clone of this repository:

```sh
cargo install --locked --path .
cwlogs-retention --help
```

From crates.io: `cargo install cwlogs-retention`. Binary archives for Linux (glibc and musl), Windows, and macOS are built by `.github/workflows/release.yml` when a `v*` tag is pushed; no tag exists yet, so there is no archive to download. Each archive would hold the binary, the man page, the READMEs, QUALITY, the licenses, and the third-party license list.

Supported systems: Linux x86_64 is built and tested. The code compiles for Windows and macOS, but the tests were not run there (see `QUALITY.md`). The Unix-only parts are the SIGINT handling and the closed-standard-output check.

`cargo test --locked` `cargo test --locked` runs 229 tests (72 unit, 27 claims, 106 CLI, 8 docs, 1 handbook, 15 README); see `QUALITY.md`.

## Quick start

`examples/describe-log-groups.json` is one invented page of `aws logs describe-log-groups`. Run this from the repository root; it only reads the file and writes nothing:

```console
$ cwlogs-retention --quiet --as-of 2026-11-09 examples/describe-log-groups.json
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

log_group             class              retention  age_days  stored_bytes   gb  estimated_usd_month  save_30_usd  save_90_usd  save_365_usd
/aws/ecs/worker       INFREQUENT_ACCESS       none        10   10737418240   10                 0.30         0.00         0.00          0.00
/aws/lambda/billing   STANDARD                  14       400   15032385536   14                 0.42         0.00         0.00          0.00
/aws/lambda/checkout  STANDARD                none       365  391915765760  365                10.95        10.05         8.25          0.00
total                                                         417685569536  389                11.67        10.05         8.25          0.00
```

`--as-of` is midnight UTC; the three groups were created 10, 400, and 365 days before 2026-11-09. `/aws/lambda/checkout` has no retention and holds 365 GB, so keeping 30 days would save 10.05 USD per month.

On your own account, pipe the export in instead of saving it first:

```sh
aws logs describe-log-groups | cwlogs-retention --as-of "$(date -u +%Y-%m-%d)" -
```

For several pages (`nextToken`), save each page into one empty directory outside this repository and pass the directory.

## Quickstart in 5 minutes

1. Install with `cargo install --locked --path .` (Rust 1.85 or newer).
2. Get your export: `aws logs describe-log-groups > /tmp/groups.json`. The file may also be `.gz`, or you give a folder or `-` (standard input).
3. Run `cwlogs-retention --as-of "$(date -u +%Y-%m-%d)" /tmp/groups.json`. Without `--as-of` the savings columns are missing.
4. Read the table: `estimated_usd_month` is the storage of the bytes already stored; `save_30_usd`, `save_90_usd` and `save_365_usd` are the saving if you keep 30, 90 or 365 days and the inflow stays the same.
5. The advice shows `aws logs put-retention-policy ...` and never runs it. A shorter retention period deletes logs you do not get back; the amount is an estimate, not an invoice and not a forecast.

The full Dutch README is in `README.nl.md`.

## Options and usage

```text
Estimate CloudWatch Logs storage cost from describe-log-groups JSON. Reads files only, no network.

Usage: cwlogs-retention [OPTIONS] <FILE>...

Arguments:
  <FILE>...  `describe-log-groups` JSON files, gzip exports, directories, or `-` for standard input

Options:
      --prices <FILE>              Price file. The default is the built-in CloudWatch Logs storage list
      --as-of <YYYY-MM-DD>         Date ages are measured to, `YYYY-MM-DD`, midnight UTC. Without it, savings are omitted
      --fail-stored-bytes <BYTES>  Exit 1 when a group with no retention has storedBytes greater than this
      --json                       Print JSON instead of the text table
      --md                         Print a Markdown table instead of the text table
      --output <FILE>              Write the report to this file instead of standard output. An existing file is replaced
  -q, --quiet                      Print only the result and errors: no notes, no warnings
  -v, --verbose                    Add progress lines on standard error
  -h, --help                       Print help
  -V, --version                    Print version

Exit codes: 0 ok, 1 unretained group above --fail-stored-bytes, 2 input or usage error, 3 --quiet with --verbose or --json with --md.
```

Input: a file of one `describe-log-groups` response (`{"logGroups": [...]}`), a bare array, a single log group object, a gzip file of one of these (recognised by its first two bytes, not the name), a directory of such files, or `-` for standard input.

A price file (`--prices`) is a JSON object with `currency`, `source`, `date`, `region`, and the three rates `standard_storage_per_gb_month`, `infrequent_access_storage_per_gb_month`, and `delivery_storage_per_gb_month` as decimal strings. The built-in one is `prices/cloudwatch-logs-storage.json`:

```json
{
  "currency": "USD",
  "source": "https://aws.amazon.com/cloudwatch/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "standard_storage_per_gb_month": "0.03",
  "infrequent_access_storage_per_gb_month": "0.03",
  "delivery_storage_per_gb_month": "0.03"
}
```

If the date of the price data (the built-in one, or `date` in a `--prices` file) is more than 180 days before `--as-of`, or before today when `--as-of` is not given, a line `warning: price data dated DATE is N days old at DAY, ...` goes to standard error and the exit code stays 0. Exactly 180 days is not older. The environment variable `CWLOGS_RETENTION_TODAY=YYYY-MM-DD` sets the day used as today, which keeps runs and tests from depending on the clock; the tests set it.

`--quiet` (`-q`) prints only the result: in text, the estimate line and the table (and the two threshold lines when `--fail-stored-bytes` is set), without notes, advice, or warnings, and no warning goes to standard error; the JSON stays whole. `--verbose` (`-v`) adds `progress:` lines on standard error and leaves standard output as it is. Both together are a usage error with exit 3 and nothing is read. Exit codes 0, 1, and 2 are unchanged.

`--md` prints the same table as Markdown: the same amounts, with no other rounding, one estimate line, the notes, and a `|`, a backslash, or a line break inside a cell escaped. `--md` with `--json` is a usage error with exit 3.

`--output FILE` writes the report to `FILE` instead of standard output, which stays empty. The report goes to a new temporary file in the same directory and is renamed over `FILE`, so `FILE` is the old report or the whole new one. An existing file is replaced. A symlink at `FILE` is replaced itself; its target is not followed and not changed. A missing directory is not created and a directory at `FILE` is refused; any failed write is exit 2 with `cwlogs-retention: could not write the report to FILE: ...` and no temporary file left. A run that fails earlier (for example an unreadable input) leaves `FILE` as it was. `--fail-stored-bytes` still gives exit 1 and still writes the file.

On Unix, Ctrl-C (SIGINT) ends the run with exit 130 and the line `cwlogs-retention: interrupted by SIGINT` on standard error. Nothing is written to standard output before the report is complete, so an interrupted run never leaves a half report or an empty one that looks like success. If the report is already being written, it is finished first and a second Ctrl-C ends the run at once.

`--json` prints the same totals, with money as strings. The first key is `"schema_version": 1`, a whole number that changes only when a key is removed or changes meaning; keys are added without a change. Every JSON document has it, with or without `--quiet`, `--verbose`, or `--output`. A failed run prints no JSON, only a message on standard error, so there is no JSON error document. The source URL and the date are in the document (see "Example").

## Example

A test runs every console block in this file and compares it to this binary. `--as-of` is midnight UTC. The three groups were created 10, 400, and 365 days before 2026-11-09.

```console
$ cwlogs-retention --as-of 2026-11-09 examples/describe-log-groups.json
prices: USD, source https://aws.amazon.com/cloudwatch/pricing/, date 2026-11-09, region us-east-1
as-of: 2026-11-09
free tier not applied: pricing examples waive the first 5 GB of archived logs per month for the account, and this export is not the whole account
not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

log_group             class              retention  age_days  stored_bytes   gb  estimated_usd_month  save_30_usd  save_90_usd  save_365_usd
/aws/ecs/worker       INFREQUENT_ACCESS       none        10   10737418240   10                 0.30         0.00         0.00          0.00
/aws/lambda/billing   STANDARD                  14       400   15032385536   14                 0.42         0.00         0.00          0.00
/aws/lambda/checkout  STANDARD                none       365  391915765760  365                10.95        10.05         8.25          0.00
total                                                         417685569536  389                11.67        10.05         8.25          0.00

advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30
advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90
advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365
advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30
advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90
advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365
note: savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.
```
```

`/aws/lambda/checkout` stores 365 GB and has no retention, so the horizon is 365 days. Keeping 30 days would drop 335 GB at 0.03 USD, which is 10.05 USD per month. Keeping 365 days drops nothing that is still stored. `/aws/lambda/billing` already keeps 14 days and the group is older than that, so the horizon is 14 days and every target saves 0.00 USD. `/aws/ecs/worker` is 10 days old, so 30, 90, and 365 days are all at or above the horizon. The account id in the sample ARN is not printed. The advice line shows `put-retention-policy` and says it was not executed.

The same report as Markdown, with `--md`:

```console
$ cwlogs-retention --md --as-of 2026-11-09 examples/describe-log-groups.json
# cwlogs-retention

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

prices: USD, source https://aws.amazon.com/cloudwatch/pricing/, date 2026-11-09, region us-east-1  
as-of: 2026-11-09

free tier not applied: pricing examples waive the first 5 GB of archived logs per month for the account, and this export is not the whole account  
not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.

| log_group | class | retention | age_days | stored_bytes | gb | estimated_usd_month | save_30_usd | save_90_usd | save_365_usd |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| /aws/ecs/worker | INFREQUENT_ACCESS | none | 10 | 10737418240 | 10 | 0.30 | 0.00 | 0.00 | 0.00 |
| /aws/lambda/billing | STANDARD | 14 | 400 | 15032385536 | 14 | 0.42 | 0.00 | 0.00 | 0.00 |
| /aws/lambda/checkout | STANDARD | none | 365 | 391915765760 | 365 | 10.95 | 10.05 | 8.25 | 0.00 |
| total |  |  |  | 417685569536 | 389 | 11.67 | 10.05 | 8.25 | 0.00 |

- advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30
- advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90
- advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365
- advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30
- advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90
- advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365

note: savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.
```

The same report as JSON, with `--json`:

```console
$ cwlogs-retention --json --as-of 2026-11-09 examples/describe-log-groups.json
{
  "schema_version": 1,
  "version": "1.0.0",
  "currency": "USD",
  "source": "https://aws.amazon.com/cloudwatch/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "as_of": "2026-11-09",
  "estimate": true,
  "estimate_note": "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)",
  "free_tier_applied": false,
  "not_priced": "not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.",
  "fail_stored_bytes": null,
  "threshold_exceeded": false,
  "note": "savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.",
  "warnings": [],
  "log_groups": [
    {
      "log_group": "/aws/ecs/worker",
      "class": "INFREQUENT_ACCESS",
      "retention_days": null,
      "age_days": "10",
      "stored_bytes": 10737418240,
      "gb": "10",
      "estimated_usd_month": "0.30",
      "save_30_usd": "0.00",
      "save_90_usd": "0.00",
      "save_365_usd": "0.00",
      "advice": [
        "advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30",
        "advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90",
        "advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365"
      ]
    },
    {
      "log_group": "/aws/lambda/billing",
      "class": "STANDARD",
      "retention_days": 14,
      "age_days": "400",
      "stored_bytes": 15032385536,
      "gb": "14",
      "estimated_usd_month": "0.42",
      "save_30_usd": "0.00",
      "save_90_usd": "0.00",
      "save_365_usd": "0.00",
      "advice": []
    },
    {
      "log_group": "/aws/lambda/checkout",
      "class": "STANDARD",
      "retention_days": null,
      "age_days": "365",
      "stored_bytes": 391915765760,
      "gb": "365",
      "estimated_usd_month": "10.95",
      "save_30_usd": "10.05",
      "save_90_usd": "8.25",
      "save_365_usd": "0.00",
      "advice": [
        "advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30",
        "advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90",
        "advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365"
      ]
    }
  ],
  "totals": {
    "stored_bytes": 417685569536,
    "gb": "389",
    "estimated_usd_month": "11.67",
    "save_30_usd": "10.05",
    "save_90_usd": "8.25",
    "save_365_usd": "0.00"
  }
}
```

## How it counts

C1. One log group object is one group. The `logGroups` array is the describe-log-groups shape. A bare array and a single object are accepted too.
C2. Monthly storage USD is storedBytes / 1024^3 times the class storage rate, worked out exactly in whole numbers (storedBytes times the rate mantissa over 1024^3 times a power of ten), with no digit cut before the end. Each row USD amount is rounded half away from zero to the cent, once. The total is the exact sum of the rows, rounded half away from zero to the cent once when shown, so a total that is exactly a half cent rounds up. The gb column shows at most 6 decimal places, with the same rounding. 1,024 GB is 1 TB, matching the pricing example. 30 TB at 0.03 USD is 921.60 USD. A directory total is rounded once from the exact amounts, so the directory can show 0.08 while the single files show 0.03 and 0.06.
C3. Standard archival is 0.03 USD per GB-month.
C4. Infrequent Access archival in the pricing example is also 0.03 USD per GB-month. A price file can set a different rate per class.
C5. Delivery archival in the pricing example is 0.03 USD per GB-month.
C6. Source URL, date, and region are printed. The built-in quote is https://aws.amazon.com/cloudwatch/pricing/ , dated 2026-11-09, region us-east-1.
C7. The 5 GB account free tier in the pricing examples is not subtracted. 6 GB of archived logs is 0.18 USD.
C8. Ingestion, Logs Insights, Live Tail, and data-protection scanning are not priced. Vended delivery tiers are not priced. Only archived storage of the stored bytes is priced.
C9. A missing retentionInDays means the group never expires. JSON null means the same.
C10. Savings use a steady-ingest model: the horizon is the age when there is no retention, or the shorter of age and retention when retention is set. Retained bytes at T days are storedBytes * min(T, horizon) / horizon. Saved USD is rounded half away from zero to the cent. This is not a forecast.
C11. Targets are 30, 90, and 365 days. A target at or above the horizon saves 0.
C12. `--as-of YYYY-MM-DD` is midnight UTC. Age is (as_of - creationTime) / 86400000 days. creationTime is epoch milliseconds. The age_days in the output has at most 6 decimal places, and the savings use the full value.
C13. Without `--as-of`, or without creationTime, storage cost is still shown and savings are omitted.
C14. `--fail-stored-bytes N` exits 1 when a group with no retention has storedBytes greater than N. A group that has retention does not trip it. The report is still printed. `--json` does not change the exit code. If a directory member was skipped (nested symlink, non-regular file, a file that is not UTF-8, or a text note), the run is an error with exit 2, because the total would be missing that member.
C15. `--json` prints money as strings, includes the source and the date, and ends with a newline.
C16. The advice line is text. It contains `put-retention-policy` and says it was not executed. The program does not run it.
C17. There is no AWS SDK and no network. nextToken is not followed. The token is not printed.
C18. Duplicate log group names are an error. The raw JSON is not printed.
C19. storedBytes must be a non-negative integer. Exponents and negatives are rejected and not echoed.
C20. Log group names are 1 to 512 characters of the CloudWatch name alphabet. A bad name is not echoed.
C21. Gzip is recognized by the magic bytes 1f 8b, not by the file name. Bytes after the first gzip member are an error.
C22. Account ids in ARNs are not printed. An entry may hold both `arn` and `logGroupArn`, as the API documents; they are accepted when they agree on the region and refused (`both arn and logGroupArn`) when they name two regions. Each region in an ARN that differs from the price region is one warning (groups from one other region give one warning, groups from two other regions give two).
C23. Rounded rows do not add up to the total. There is no per-day ingest extrapolation. The monthly cost is the stored bytes already counted.
C24. Errors do not include file contents. Control, invisible, and direction-changing characters in paths, names, the price source, and the arguments echoed in a usage error are printed as `\u{...}` escapes, a newline as `\u{a}`, so an error is one line. In a path or an argument that is not UTF-8, each invalid byte is printed as `\xNN` and the rest is escaped the same way.

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | The report was printed (also with warnings, and with `--quiet`). |
| 1 | A log group with no retention has more stored bytes than `--fail-stored-bytes`. The report is still printed or written. |
| 2 | Input or usage error: an unreadable or invalid file, a duplicate name, a bad price file, an amount that does not fit (`price overflow`), a failed write of the report, or a closed or full standard output. The message is one line on standard error, with control characters escaped, and holds no file contents. |
| 3 | `--quiet` with `--verbose`, or `--json` with `--md`. Nothing is read. |
| 130 | Interrupted by Ctrl-C (SIGINT, Unix only). Nothing was written to standard output. |

A failed run prints no JSON, only the message on standard error.

## Limitations

The advice prints `aws logs put-retention-policy` and says the command was not executed. The program does not run it. A shorter retention deletes logs you cannot get back. The savings figure is not a forecast.

The built-in rates are the us-east-1 examples on the CloudWatch pricing page, read on 2026-11-09. Another region needs `--prices`. The account free tier in those examples is not applied, because one export is not the whole account.

Ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers are outside this tool. There is no per day column. The monthly figure is the storage of the bytes already stored.

The tool has no AWS SDK and opens no network. A nextToken is not followed, so one response page is not the whole account. The token is not printed.

A file larger than 268435456 bytes is rejected. A nested symlink is skipped with a warning. With `--json`, warnings go to standard error. An explicit symlink is followed. Passing a directory that is nested too deeply, past 64 levels, is an error: `directory is nested too deeply`.

If standard output is closed or a write to it fails, including for `--help` and `--version`, the tool exits 2 with `cwlogs-retention: could not write output` on standard error. With a closed standard output, a usage error or an input error is still shown on standard error (also exit 2). A closed standard output is detected on Linux only. Standard output on `/dev/null` still exits 0. A failed write of a warning to standard error is ignored.

### Input rules

- A log group needs a `logGroupName` of 1 to 512 characters from the CloudWatch name alphabet and a `storedBytes` that is a plain run of digits, as a number or as a string. In a string only space, tab, CR, and LF around the digits are ignored; no other blank counts.
- `retentionInDays` is a whole number from 1 to 3653, as a number or as a string of digits (`"14"` and `"0014"` are 14; `"+14"` and `"1.0"` are `bad retentionInDays`). A number in that range that CloudWatch does not offer, such as 8, is shown with a warning and is not changed.
- Arrays and objects nested 128 or more levels deep are `not JSON`; 127 levels are read. The envelope, the array, and the group count as levels.
- A number given as a string is at most 20 characters, also with leading zeros: `"018446744073709551615"` (21 characters) is `bad storedBytes`, and so is a `retentionInDays` string of 21 or more characters, although `"0014"` is 14.
- `storedBytes` is at most 18446744073709551615 (a 64-bit unsigned number). The total of all groups has the same limit: two groups of 18446744073709551615 bytes are `stored bytes overflow` and exit 2, and the message names no file or group.
- A file with a byte order mark is read; UTF-16 is `not UTF-8`.
- A price file value is a plain decimal of at most 40 characters, with at most 28 digits after the point and at most 28 significant digits. Anything else is `bad` plus the field name, because a longer number would be rounded to a different rate. There is no upper limit on a rate: 1000000000 USD per GB is accepted and printed. An amount that does not fit is `price overflow`.
- In a directory, a member that is a text file, is not UTF-8, or is empty is skipped with a warning. A member that is valid JSON without log groups, such as `42`, is not skipped: it is `expected log groups` and exit 2. With `--fail-stored-bytes`, any skipped member is exit 2.
- The price `source` shown with `--json` is the escaped text: a source with U+202E is `x\\u{202e}y` in the JSON document.

### Money precision

- Amounts are computed exactly and rounded once to the cent, half away from zero: the monthly amount of a row, the total of all rows, each saving, and the total of the savings. Bytes, the rate (as a mantissa and a power of ten), and the age of a group (in milliseconds, as a fraction of a day) are multiplied and divided as whole numbers of any size, and the rows of a total are added without rounding them first. The printed amount is the exact amount for every rate the tool accepts. Examples that were one cent too low before: three groups of 161730679452, 561861173778, and 162782022482 bytes at the built-in rate total `24.77` (exactly 24.765), and a group of 492042190848 bytes with a `creationTime` 94/3 days before `--as-of 2026-11-09` saves `0.59` in 30 days (exactly 0.585). The tests compare random groups, ages, and retentions with whole-number arithmetic.
- The `gb` figure and the age are shown with at most 6 decimals, half away from zero; they are not money and do not enter an amount.
- An amount that the decimal type cannot hold after rounding to the cent is `price overflow` (exit 2), never a cut amount. That is more than 792281625142643375935439503.35 USD with cents, or a whole-dollar amount of more than 79228162514264337593543950335 USD. For example a rate of `1000000000000000000` USD per GB with 18446744073709551615 bytes is `price overflow`; it printed `17179869183999999999068677425.00` before, where the exact amount is `...425.38`.
- Cost: a total is added as scaled whole numbers, one step per group, and a row costs the same however many rows came before. When that scaled sum could fall on either side of a cent, the total is decided from the exact amounts. Rows with the same denominator (the same age and rate) are added into one numerator as they are read, those numerators are reduced, and they are added over their least common multiple. This exact step is bounded: at most 4096 different denominators, and a common denominator of at most 2048 bits. A total that is that close to a half cent and goes past either bound is the error `total is too close to a half cent to round exactly within the work limit` (exit 2); it is never rounded by a guess. An exact half cent over up to 4096 different ages, or over any number of groups that share their ages in pairs or in larger sets, is rounded half away from zero as usual. Measured with the release binary, the text table, and `--as-of 2026-11-09`, the shorter of two runs and the higher peak. 100000, 200000, and 400000 groups with a different age each, where the cent is certain from the scaled sum: 1.71 s and 152876 KB, 3.53 s and 305268 KB, 6.85 s and 566644 KB; from the 100000-group run, time grew by 2.06 and 4.00, and peak memory by 2.00 and 3.71. 1000, 2000, and 4000 pairs of groups (2000, 4000, and 8000 groups) whose pairs have distinct ages and whose savings total is an exact half cent, so that the exact step runs: 0.04 s and 17020 KB, 0.08 s and 30724 KB, 0.17 s and 58712 KB; from the first run, time grew by 2.0 and 4.3, and peak memory by 1.8 and 3.4. The same kind of file with 50000 pairs (100000 groups) has more than 4096 different denominators and ends with the error above after 0.73 s and 94608 KB.

### Size, time, and system

- One run read a 268434299-byte file with 1348916 small groups in 10 seconds and used 2.7 GB of memory at its peak (release build, one Linux machine). Bytes are dropped after each file is parsed, so a directory does not add up its files in memory.
- Memory is not bounded by the file size. Every field, also one the tool does not use, is held in memory while the file is read: a file with one group and an unknown field that holds a list of numbers took 0.7 seconds and 0.53 GB at 10000059 bytes, 2.7 seconds and 2.1 GB at 40000059 bytes, and 6.9 seconds and 5.3 GB at 100000059 bytes, about 53 times the file size. A file near the size limit can therefore need more memory than the machine has. With `--json`, 1 million groups (69777795 bytes) took 4.6 GB, against 0.9 GB for the text table. Keep exports to the groups you need.
- A gzip file inside a gzip file is `not UTF-8`, not a gzip error. The error for a `creationTime` after `--as-of` names the log group but not the file.
- Built and tested on Linux x86_64 with glibc; the release binary built here needs glibc 2.34 or newer, and no older glibc was tried. A musl build runs the sample and the closed and full standard output cases; the test suite was not run on musl. The code compiles for Windows and macOS (clippy), but the tests were not run there.

## Prices and responsibility

The built-in prices are a snapshot of https://aws.amazon.com/cloudwatch/pricing/ dated 2026-11-09 (the `date` in `prices/cloudwatch-logs-storage.json`). Check the current prices yourself. You are responsible for any decision you take on the basis of the output. Use `--prices` to give your own price file.

## Maintenance

Report bugs through GitHub Issues of this repository. After a release the tool is in dormant mode. A new version follows only for a reported bug, a request, or a security problem; security always comes first.

## Support

Report a bug or ask a question through [GitHub Issues](https://github.com/qwpp7g79y4-maker/cwlogs-retention/issues). Include the version, the command, and a minimal invented input, never a real export. For security problems use the private route in `SECURITY.md`. To contribute, read `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`. The licenses of the third-party crates in a release binary are listed in the file `THIRD-PARTY-LICENSES.html` of each release archive.

The licenses of the third-party crates built into the binary are listed in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Website: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
