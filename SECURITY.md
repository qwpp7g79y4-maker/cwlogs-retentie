# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

Security fixes go into the latest release only. The crate is not published and has no git tag.

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and choose **Report a vulnerability**. If that is not possible, write to pep_ai@icloud.com. Examples are a file name, log group name, or price file field that can add a fake line or a terminal escape to the report or to an error, an error that echoes file contents it should hide, a way to make cwlogs-retentie write, run, or connect to something, a file that makes it read without the stated size limit or hang, and input that changes an amount without an error.

Memory use of up to about 53 times the file size is a documented limit (see the README, "Size, time, and system"), not a vulnerability by itself. Amounts are exact and rounded once to the cent; an amount that does not fit is an error and not a cut amount (see the README, "Money precision"). The exact decision for a total that lands on a half cent has a fixed work limit (4096 different denominators, 2048 bits); past it the run ends with an error, exit 2, and a file that makes it last longer than the times in the README is reportable.

Please include:

- the cwlogs-retentie version (the `version` field in `Cargo.toml`, or the commit you built) and your operating system;
- the exact command;
- a minimal synthetic `describe-log-groups` file or `.gz` that reproduces the problem, never a real export;
- what cwlogs-retentie printed and what you expected.

We aim to respond within 7 days (best effort). Once a fix is released, the report can be credited in the changelog if you wish.
