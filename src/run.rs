use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rust_decimal::Decimal;

use crate::bill::{savings_frac, Bill, Horizon, Row, Totals};
use crate::error::{escape_controls, escape_os_str, Error};
use crate::money::{gb_from_bytes, storage_frac, ymd_to_epoch_millis, Frac, Sum};
use crate::parse::{parse_document, Group};
use crate::price::{load_prices_str, prices_builtin, Prices};
use crate::report::{render_with, Format};
use crate::MAX_FILE_BYTES;

/// What to price, and how to print it.
#[derive(Debug, Clone)]
pub struct Options {
    pub prices: Prices,
    pub as_of: Option<String>,
    pub fail_stored_bytes: Option<u64>,
    pub json: bool,
    /// Markdown instead of text; `json` wins when both are set.
    pub markdown: bool,
    /// Text output holds only the estimate line, the table, and the threshold lines.
    pub quiet: bool,
    /// Progress lines on standard error while reading and pricing.
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prices: prices_builtin(),
            as_of: None,
            fail_stored_bytes: None,
            json: false,
            markdown: false,
            quiet: false,
            verbose: false,
        }
    }
}

/// One file, or one member of a directory walk (`strict` is false for members).
pub struct NamedBytes {
    pub name: String,
    pub bytes: Vec<u8>,
    pub strict: bool,
}

/// Rendered report. `warnings` is also included in `stdout` for text output.
#[derive(Debug)]
pub struct Output {
    pub stdout: String,
    pub warnings: Vec<String>,
    /// Set when the price data is older than [`PRICE_MAX_AGE_DAYS`]; meant for standard error.
    pub price_age_warning: Option<String>,
    pub threshold_exceeded: bool,
}

/// A price date more than this many days before the reference date is reported.
pub const PRICE_MAX_AGE_DAYS: i64 = 180;

/// Warning for price data older than 180 days at the reference day, or `None`.
///
/// The reference day is `as_of` when it is a valid date, else `today_ms` (Unix
/// milliseconds, passed in so tests never read the clock). A future price date and
/// an unreadable reference are not reported.
#[must_use]
pub fn price_age_warning(
    prices_date: &str,
    as_of: Option<&str>,
    today_ms: Option<i64>,
) -> Option<String> {
    let price_ms = ymd_to_epoch_millis(prices_date)?;
    let (reference_ms, reference_text) = match as_of {
        Some(text) => (ymd_to_epoch_millis(text)?, text.to_string()),
        None => {
            let days = today_ms?.div_euclid(86_400_000);
            (days * 86_400_000, civil_from_days_text(days))
        }
    };
    let age_days = reference_ms.checked_sub(price_ms)?.div_euclid(86_400_000);
    if age_days > PRICE_MAX_AGE_DAYS {
        Some(format!(
            "price data dated {prices_date} is {age_days} days old at {reference_text}, more than {PRICE_MAX_AGE_DAYS} days; check the current list prices"
        ))
    } else {
        None
    }
}

fn civil_from_days_text(days: i64) -> String {
    let (year, month, day) = crate::money::civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Today as Unix milliseconds: the valid date in `CWLOGS_RETENTION_TODAY`, else the clock.
fn now_millis() -> Option<i64> {
    if let Some(day) = std::env::var_os("CWLOGS_RETENTION_TODAY") {
        if let Some(ms) = day.to_str().and_then(ymd_to_epoch_millis) {
            return Some(ms);
        }
    }
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(elapsed.as_millis()).ok()
}

pub fn bill_from_inputs(inputs: &[NamedBytes], options: &Options) -> Result<Bill, Error> {
    let as_of_ms = match &options.as_of {
        Some(text) => Some(
            ymd_to_epoch_millis(text)
                .ok_or_else(|| Error::new("invalid --as-of: expected YYYY-MM-DD"))?,
        ),
        None => None,
    };
    let mut acc = Parsed::default();
    for input in inputs {
        acc.add(&input.name, &input.bytes, input.strict)?;
    }
    acc.finish(options, as_of_ms)
}

/// Parsed groups so far. Raw bytes are dropped after each file is added.
#[derive(Default)]
struct Parsed {
    groups: Vec<Group>,
    warnings: Vec<String>,
    /// Members left out of the total, whatever their warning text starts with.
    skipped: Vec<String>,
}

impl Parsed {
    fn add(&mut self, name: &str, bytes: &[u8], strict: bool) -> Result<(), Error> {
        let parsed = parse_document(name, bytes, strict)?;
        self.warnings.extend(parsed.warnings);
        self.skipped.extend(parsed.skipped);
        self.groups.extend(parsed.groups);
        Ok(())
    }

    fn finish(self, options: &Options, as_of_ms: Option<i64>) -> Result<Bill, Error> {
        if self.groups.is_empty() {
            return Err(Error::new("no log groups"));
        }
        price_groups(self.groups, options, as_of_ms, self.warnings)
    }
}

pub fn execute_paths(paths: &[PathBuf], options: &Options) -> Result<Output, Error> {
    let as_of_ms = match &options.as_of {
        Some(text) => Some(
            ymd_to_epoch_millis(text)
                .ok_or_else(|| Error::new("invalid --as-of: expected YYYY-MM-DD"))?,
        ),
        None => None,
    };
    let mut acc = Parsed::default();
    let mut warnings = Vec::new();
    for path in paths {
        progress(options, &format!("reading {}", path_label(path)));
        if path.as_os_str() == "-" {
            let mut buf = Vec::new();
            std::io::stdin()
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut buf)
                .map_err(|err| Error::new(format!("-: {err}")))?;
            if buf.len() as u64 > MAX_FILE_BYTES {
                return Err(Error::new("-: file is larger than 268435456 bytes"));
            }
            acc.add("-", &buf, true)?;
            continue;
        }
        push_explicit(path, &mut acc, &mut warnings)?;
    }
    let skipped: Vec<String> = acc
        .skipped
        .iter()
        .map(|line| escape_controls(line))
        .collect();
    let mut bill = acc.finish(options, as_of_ms)?;
    let mut all = warnings;
    all.append(&mut bill.warnings);
    // One escape pass; escaping never touches a backslash, so it is idempotent.
    bill.warnings = all.iter().map(|w| escape_controls(w)).collect();
    if options.fail_stored_bytes.is_some() {
        if let Some(skip) = skipped.first() {
            return Err(Error::new(format!(
                "{skip}; --fail-stored-bytes cannot be judged because this member is missing from the total"
            )));
        }
    }
    let threshold_exceeded = bill.threshold_exceeded;
    let warnings = bill.warnings.clone();
    let inputs = paths.len();
    progress(
        options,
        &format!(
            "priced {} log group{} from {inputs} input{}",
            bill.rows.len(),
            if bill.rows.len() == 1 { "" } else { "s" },
            if inputs == 1 { "" } else { "s" }
        ),
    );
    let format = if options.json {
        Format::Json
    } else if options.markdown {
        Format::Markdown
    } else {
        Format::Text
    };
    let stdout = render_with(&bill, format, options.quiet);
    let price_age_warning =
        price_age_warning(&options.prices.date, options.as_of.as_deref(), now_millis());
    Ok(Output {
        stdout,
        warnings,
        price_age_warning,
        threshold_exceeded,
    })
}

/// One progress line on standard error; a failed write is ignored.
fn progress(options: &Options, message: &str) {
    if options.verbose {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "progress: {message}");
    }
}

fn path_label(path: &Path) -> String {
    escape_os_str(path.as_os_str())
}

fn push_explicit(path: &Path, acc: &mut Parsed, warnings: &mut Vec<String>) -> Result<(), Error> {
    let (label, meta) = followed_meta(path)?;
    if meta.is_dir() {
        return collect_dir(path, 1, acc, warnings);
    }
    if !meta.is_file() {
        return Err(Error::new(format!("{label}: not a file")));
    }
    let bytes = read_limited(path)?;
    acc.add(&label, &bytes, true)
}

/// Follow one explicit symlink, then return the target metadata.
fn followed_meta(path: &Path) -> Result<(String, fs::Metadata), Error> {
    let label = path_label(path);
    let symlink_meta =
        fs::symlink_metadata(path).map_err(|err| Error::new(format!("{label}: {err}")))?;
    let meta = if symlink_meta.file_type().is_symlink() {
        fs::metadata(path).map_err(|err| Error::new(format!("{label}: {err}")))?
    } else {
        symlink_meta
    };
    Ok((label, meta))
}

/// Read a price file the same way as one input file: a regular file, with the byte limit.
pub fn load_prices_file(path: &Path) -> Result<Prices, Error> {
    let (label, meta) = followed_meta(path)?;
    if !meta.is_file() {
        return Err(Error::new(format!("{label}: not a file")));
    }
    let bytes = read_limited(path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| Error::new(format!("{label}: not UTF-8")))?;
    load_prices_str(text).map_err(|err| Error::new(format!("price file {label}: {err}")))
}

fn collect_dir(
    dir: &Path,
    depth: usize,
    acc: &mut Parsed,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::new(format!(
            "{}: directory is nested too deeply",
            path_label(dir)
        )));
    }
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("{}: {err}", path_label(dir))))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("{}: {err}", path_label(dir))))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        let label = path_label(&path);
        let meta =
            fs::symlink_metadata(&path).map_err(|err| Error::new(format!("{label}: {err}")))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let line = format!("skipped symlink {label}");
            acc.skipped.push(line.clone());
            warnings.push(line);
            continue;
        }
        if kind.is_dir() {
            collect_dir(&path, depth + 1, acc, warnings)?;
        } else if kind.is_file() {
            let bytes = read_limited(&path)?;
            acc.add(&label, &bytes, false)?;
        } else {
            let line = format!("skipped non-regular file {label}");
            acc.skipped.push(line.clone());
            warnings.push(line);
        }
    }
    Ok(())
}

fn read_limited(path: &Path) -> Result<Vec<u8>, Error> {
    let label = path_label(path);
    let meta = fs::metadata(path).map_err(|err| Error::new(format!("{label}: {err}")))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{label}: file is larger than 268435456 bytes"
        )));
    }
    let file = fs::File::open(path).map_err(|err| Error::new(format!("{label}: {err}")))?;
    let mut buf = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|err| Error::new(format!("{label}: {err}")))?;
    if buf.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{label}: file is larger than 268435456 bytes"
        )));
    }
    Ok(buf)
}

fn price_groups(
    groups: Vec<Group>,
    options: &Options,
    as_of_ms: Option<i64>,
    mut warnings: Vec<String>,
) -> Result<Bill, Error> {
    let mut ordered: BTreeMap<String, Group> = BTreeMap::new();
    for group in groups {
        let name = group.name.clone();
        if ordered.insert(name.clone(), group).is_some() {
            return Err(Error::new(format!("duplicate log group {name}")));
        }
    }
    let mut rows = Vec::with_capacity(ordered.len());
    let mut stored_total: u64 = 0;
    let mut usd_total = Sum::new();
    let mut save_totals = [Some(Sum::new()), Some(Sum::new()), Some(Sum::new())];
    let mut seen_regions = BTreeSet::new();
    for (name, group) in ordered {
        if let Some(region) = &group.region {
            if region != &options.prices.region && seen_regions.insert(region.clone()) {
                warnings.push(format!(
                    "log group region {region} differs from price region {}",
                    options.prices.region
                ));
            }
        }
        let rate = options.prices.rate(group.class);
        let gb = gb_from_bytes(Decimal::from(group.stored_bytes));
        let usd_exact = storage_frac(group.stored_bytes, rate);
        let aged = savings_for(&group, as_of_ms, rate, &mut warnings)?;
        stored_total = stored_total
            .checked_add(group.stored_bytes)
            .ok_or_else(|| Error::new("stored bytes overflow"))?;
        usd_total.add(&usd_exact);
        let mut saves = [None, None, None];
        for (index, exact) in aged.saves.iter().enumerate() {
            match exact {
                Some(frac) => {
                    if let Some(sum) = &mut save_totals[index] {
                        sum.add(frac);
                    }
                    saves[index] = Some(frac.round_cents()?);
                }
                None => save_totals[index] = None,
            }
        }
        rows.push(Row {
            name,
            class: group.class,
            retention_days: group.retention_days,
            age_days: aged.age_days,
            stored_bytes: group.stored_bytes,
            gb,
            usd_month: usd_exact.round_cents()?,
            save_30_usd: saves[0],
            save_90_usd: saves[1],
            save_365_usd: saves[2],
        });
    }
    let [save_30, save_90, save_365] = save_totals;
    let totals = Totals {
        stored_bytes: stored_total,
        gb: gb_from_bytes(Decimal::from(stored_total)),
        usd_month: usd_total.round_cents()?,
        save_30_usd: save_30.map(|sum| sum.round_cents()).transpose()?,
        save_90_usd: save_90.map(|sum| sum.round_cents()).transpose()?,
        save_365_usd: save_365.map(|sum| sum.round_cents()).transpose()?,
    };
    let threshold_exceeded = match options.fail_stored_bytes {
        Some(limit) => rows
            .iter()
            .any(|row| row.retention_days.is_none() && row.stored_bytes > limit),
        None => false,
    };
    Ok(Bill {
        prices: options.prices.clone(),
        as_of: options.as_of.clone(),
        fail_stored_bytes: options.fail_stored_bytes,
        threshold_exceeded,
        rows,
        warnings,
        totals,
    })
}

struct AgedSavings {
    age_days: Option<Decimal>,
    /// Exact savings for 30, 90, and 365 days.
    saves: [Option<Frac>; 3],
}

fn savings_for(
    group: &Group,
    as_of_ms: Option<i64>,
    rate: Decimal,
    warnings: &mut Vec<String>,
) -> Result<AgedSavings, Error> {
    let Some(as_of_ms) = as_of_ms else {
        return Ok(AgedSavings::unknown());
    };
    let Some(created) = group.creation_ms else {
        return Ok(AgedSavings::unknown());
    };
    let Some(delta) = as_of_ms.checked_sub(created) else {
        return Err(Error::new(format!("{}: bad creationTime", group.name)));
    };
    if delta < 0 {
        return Err(Error::new(format!(
            "{}: creationTime is after --as-of",
            group.name
        )));
    }
    let age = Decimal::from(delta) / Decimal::from(MS_PER_DAY);
    if age.is_zero() && group.stored_bytes > 0 {
        warnings.push(format!(
            "{}: age is zero; savings not estimated",
            group.name
        ));
        return Ok(AgedSavings {
            age_days: Some(age),
            ..AgedSavings::unknown()
        });
    }
    // The horizon is the age, `delta / MS_PER_DAY` days, or the retention when that is
    // shorter. It stays a fraction, so the age is not cut to 28 digits.
    let age_ms = delta as u128;
    let horizon = match group.retention_days {
        Some(days) if u128::from(days) * u128::from(MS_PER_DAY.unsigned_abs()) <= age_ms => {
            Horizon {
                num: u128::from(days),
                den: 1,
            }
        }
        _ => Horizon {
            num: age_ms,
            den: u128::from(MS_PER_DAY.unsigned_abs()),
        },
    };
    let mut saves = [None, None, None];
    for (slot, target) in saves.iter_mut().zip([30, 90, 365]) {
        *slot = savings_frac(group.stored_bytes, horizon, target, rate)?;
    }
    Ok(AgedSavings {
        age_days: Some(age),
        saves,
    })
}

const MS_PER_DAY: i64 = 86_400_000;

impl AgedSavings {
    fn unknown() -> Self {
        Self {
            age_days: None,
            saves: [None, None, None],
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn price_age_uses_as_of_or_the_passed_today_and_never_the_clock() {
        use super::price_age_warning;
        let today = crate::money::epoch_millis(2027, 5, 9);
        // 2026-11-09 to 2027-05-08 is 180 days: not older. One day more is.
        assert_eq!(
            price_age_warning("2026-11-09", Some("2027-05-08"), None),
            None
        );
        let warning = price_age_warning("2026-11-09", Some("2027-05-09"), None).unwrap();
        assert!(warning.contains("2026-11-09") && warning.contains("181 days"));
        assert!(warning.contains("2027-05-09"));
        // As-of wins over today.
        assert_eq!(
            price_age_warning("2026-11-09", Some("2026-11-10"), Some(today)),
            None
        );
        // Without as-of the passed day is the reference, also mid-day.
        let warning = price_age_warning("2026-11-09", None, Some(today + 43_200_000)).unwrap();
        assert!(
            warning.contains("181 days") && warning.contains("2027-05-09"),
            "{warning}"
        );
        assert_eq!(
            price_age_warning("2026-11-09", None, Some(today - 86_400_000)),
            None
        );
        // No reference, a future price date, or a bad date: no warning.
        assert_eq!(price_age_warning("2026-11-09", None, None), None);
        assert_eq!(
            price_age_warning("2030-01-01", Some("2026-01-01"), None),
            None
        );
        assert_eq!(price_age_warning("2026-11-09", Some("bad"), None), None);
    }

    use super::{bill_from_inputs, NamedBytes, Options};
    use crate::money::format_usd;
    use crate::report::render;

    fn inputs(json: &str) -> NamedBytes {
        NamedBytes {
            name: "f.json".to_string(),
            bytes: json.as_bytes().to_vec(),
            strict: true,
        }
    }

    fn opts(as_of: Option<&str>, fail: Option<u64>) -> Options {
        Options {
            as_of: as_of.map(str::to_string),
            fail_stored_bytes: fail,
            ..Options::default()
        }
    }

    #[test]
    fn example_file_prices_the_three_groups() {
        let json = include_str!("../examples/describe-log-groups.json");
        let options = opts(Some("2026-11-09"), None);
        let bill = bill_from_inputs(&[inputs(json)], &options).unwrap();
        assert_eq!(bill.rows[0].name, "/aws/ecs/worker");
        assert_eq!(bill.rows[1].name, "/aws/lambda/billing");
        assert_eq!(bill.rows[2].name, "/aws/lambda/checkout");
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.30");
        assert_eq!(format_usd(bill.rows[1].usd_month), "0.42");
        assert_eq!(format_usd(bill.rows[2].usd_month), "10.95");
        assert_eq!(format_usd(bill.rows[2].save_30_usd.unwrap()), "10.05");
        assert_eq!(format_usd(bill.rows[2].save_90_usd.unwrap()), "8.25");
        assert_eq!(format_usd(bill.rows[2].save_365_usd.unwrap()), "0.00");
        assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "0.00");
        assert_eq!(format_usd(bill.rows[1].save_30_usd.unwrap()), "0.00");
        assert_eq!(format_usd(bill.totals.usd_month), "11.67");
        assert_eq!(format_usd(bill.totals.save_30_usd.unwrap()), "10.05");
        assert_eq!(format_usd(bill.totals.save_90_usd.unwrap()), "8.25");
        assert_eq!(format_usd(bill.totals.save_365_usd.unwrap()), "0.00");
        assert_eq!(bill.totals.stored_bytes, 417_685_569_536);
        let text = render(&bill, false);
        assert!(!text.contains("123456789012"));
        assert!(!text.contains("per day"));
        assert!(!text.contains("factor:"));
        assert_eq!(text.matches("put-retention-policy").count(), 6);
        assert!(bill.rows[1].retention_days.is_some());
        let billing_advice = text
            .lines()
            .filter(|line| line.starts_with("advice /aws/lambda/billing"))
            .count();
        assert_eq!(billing_advice, 0);
    }

    #[test]
    fn threshold_ignores_retained_groups_and_equal_bytes() {
        let json = include_str!("../examples/describe-log-groups.json");
        let equal = opts(Some("2026-11-09"), Some(391_915_765_760));
        let bill = bill_from_inputs(&[inputs(json)], &equal).unwrap();
        assert!(!bill.threshold_exceeded);
        let over = opts(Some("2026-11-09"), Some(391_915_765_759));
        let bill = bill_from_inputs(&[inputs(json)], &over).unwrap();
        assert!(bill.threshold_exceeded);
        let retained = r#"{"logGroupName":"/a","storedBytes":99,"retentionInDays":14}"#;
        let kept = opts(Some("2026-11-09"), Some(0));
        let bill = bill_from_inputs(&[inputs(retained)], &kept).unwrap();
        assert!(!bill.threshold_exceeded);
        let open = r#"{"logGroupName":"/a","storedBytes":1}"#;
        let bill = bill_from_inputs(&[inputs(open)], &kept).unwrap();
        assert!(bill.threshold_exceeded);
    }

    #[test]
    fn savings_need_an_as_of_date_and_a_creation_time() {
        let json = r#"{"logGroupName":"/a","storedBytes":1073741824,"creationTime":1762646400000}"#;
        let bill = bill_from_inputs(&[inputs(json)], &Options::default()).unwrap();
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
        assert!(bill.rows[0].save_30_usd.is_none());
        assert!(bill.rows[0].age_days.is_none());
        let text = render(&bill, false);
        assert!(text.contains("as-of: not set"));
        let options = opts(Some("2026-11-09"), None);
        let missing = r#"{"logGroupName":"/a","storedBytes":1073741824}"#;
        let bill = bill_from_inputs(&[inputs(missing)], &options).unwrap();
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
        assert!(bill.rows[0].save_30_usd.is_none());
        assert!(bill.totals.save_30_usd.is_none());
    }

    #[test]
    fn no_log_groups_error_has_no_file_name() {
        let empty = NamedBytes {
            name: "f.json".to_string(),
            bytes: br#"{"logGroups":[]}"#.to_vec(),
            strict: false,
        };
        let err = bill_from_inputs(&[empty], &Options::default()).unwrap_err();
        assert_eq!(err.message(), "no log groups");
    }

    #[test]
    fn creation_time_overflowing_the_age_is_a_bad_creation_time() {
        let json = format!(
            r#"{{"logGroupName":"/a","storedBytes":1,"creationTime":{}}}"#,
            i64::MAX
        );
        let options = opts(Some("0000-01-01"), None);
        let err = bill_from_inputs(&[inputs(&json)], &options).unwrap_err();
        assert_eq!(err.message(), "/a: bad creationTime");
    }

    #[test]
    fn age_zero_and_creation_after_as_of() {
        let as_of = crate::money::epoch_millis(2026, 11, 9);
        let options = opts(Some("2026-11-09"), None);
        let zero = format!(r#"{{"logGroupName":"/a","storedBytes":5,"creationTime":{as_of}}}"#);
        let bill = bill_from_inputs(&[inputs(&zero)], &options).unwrap();
        assert!(bill.rows[0].save_30_usd.is_none());
        assert_eq!(bill.rows[0].age_days.unwrap(), rust_decimal::Decimal::ZERO);
        assert!(bill.warnings.iter().any(|w| w.contains("age is zero")));
        let empty = format!(r#"{{"logGroupName":"/a","storedBytes":0,"creationTime":{as_of}}}"#);
        let bill = bill_from_inputs(&[inputs(&empty)], &options).unwrap();
        assert_eq!(
            bill.rows[0].save_30_usd.unwrap(),
            rust_decimal::Decimal::ZERO
        );
        assert!(bill.warnings.is_empty());
        let after = format!(
            r#"{{"logGroupName":"/a","storedBytes":1,"creationTime":{}}}"#,
            as_of + 1
        );
        let err = bill_from_inputs(&[inputs(&after)], &options).unwrap_err();
        assert_eq!(err.message(), "/a: creationTime is after --as-of");
        assert!(!err.message().contains(&format!("{}", as_of + 1)));
    }

    #[test]
    fn duplicate_name_does_not_include_the_body() {
        let json = r#"{"logGroups":[
            {"logGroupName":"/a","storedBytes":1,"note":"RAWJSONSECRET"},
            {"logGroupName":"/a","storedBytes":2}
        ]}"#;
        let err = bill_from_inputs(&[inputs(json)], &Options::default()).unwrap_err();
        assert_eq!(err.message(), "duplicate log group /a");
        assert!(!err.message().contains("RAWJSONSECRET"));
    }

    #[test]
    fn half_day_age_is_exact() {
        let as_of = crate::money::epoch_millis(2026, 11, 9);
        let created = as_of - 43_200_000;
        let json =
            format!(r#"{{"logGroupName":"/a","storedBytes":1073741824,"creationTime":{created}}}"#);
        let options = opts(Some("2026-11-09"), None);
        let bill = bill_from_inputs(&[inputs(&json)], &options).unwrap();
        assert_eq!(
            crate::money::format_dec(bill.rows[0].age_days.unwrap()),
            "0.5"
        );
        assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "0.00");
    }

    #[test]
    fn two_u64_max_groups_overflow_without_the_digits() {
        let json = r#"{"logGroups":[
            {"logGroupName":"/a","storedBytes":18446744073709551615},
            {"logGroupName":"/b","storedBytes":1}
        ]}"#;
        let err = bill_from_inputs(&[inputs(json)], &Options::default()).unwrap_err();
        assert_eq!(err.message(), "stored bytes overflow");
        assert!(!err.message().contains("18446744073709551615"));
    }

    #[test]
    fn invalid_as_of_is_rejected() {
        let options = opts(Some("2025-02-29"), None);
        let err = bill_from_inputs(
            &[inputs(r#"{"logGroupName":"/a","storedBytes":1}"#)],
            &options,
        )
        .unwrap_err();
        assert_eq!(err.message(), "invalid --as-of: expected YYYY-MM-DD");
    }

    #[test]
    fn usd_rounds_half_away_from_zero_to_the_cent() {
        let half = r#"{"logGroups":[
            {"logGroupName":"/a","storedBytes":536870912},
            {"logGroupName":"/b","storedBytes":536870912}
        ]}"#;
        let bill = bill_from_inputs(&[inputs(half)], &Options::default()).unwrap();
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.02");
        assert_eq!(format_usd(bill.rows[1].usd_month), "0.02");
        assert_eq!(format_usd(bill.totals.usd_month), "0.03");
        let text = render(&bill, false);
        let total = text.lines().find(|line| line.starts_with("total")).unwrap();
        assert!(total.contains("0.03"), "{total}");
        assert!(!total.contains("0.04"), "{total}");
        assert!(!text.contains("0.015"));

        let three = r#"{"logGroups":[
            {"logGroupName":"/a","storedBytes":536870912},
            {"logGroupName":"/b","storedBytes":536870912},
            {"logGroupName":"/c","storedBytes":536870912}
        ]}"#;
        let bill = bill_from_inputs(&[inputs(three)], &Options::default()).unwrap();
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.02");
        assert_eq!(format_usd(bill.rows[1].usd_month), "0.02");
        assert_eq!(format_usd(bill.rows[2].usd_month), "0.02");
        assert_eq!(format_usd(bill.totals.usd_month), "0.05");

        let odd = r#"{"logGroupName":"/a","storedBytes":123456789}"#;
        let bill = bill_from_inputs(&[inputs(odd)], &Options::default()).unwrap();
        assert_eq!(format_usd(bill.rows[0].usd_month), "0.00");
        let text = render(&bill, false);
        assert!(text.contains("0.114978"));
        assert!(!text.contains("0.0034493428375571966171264648"));
        assert!(!text.contains("0.1149780945852398872375488281"));
        let json = render(&bill, true);
        assert!(!json.contains("0.0034493428375571966171264648"));
        assert!(!json.contains("0.1149780945852398872375488281"));

        let aged = r#"{"logGroupName":"/b","storedBytes":1073741824,"creationTime":1762646400000}"#;
        let bill = bill_from_inputs(&[inputs(aged)], &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "0.03");
        assert_eq!(format_usd(bill.rows[0].save_90_usd.unwrap()), "0.02");
        assert_eq!(format_usd(bill.rows[0].save_365_usd.unwrap()), "0.00");
        let text = render(&bill, false);
        assert!(text.contains("saves 0.03 USD/month"));
        assert!(text.contains("saves 0.02 USD/month"));
        assert!(!text.contains("0.0275342465753424657534246575"));

        let as_of = crate::money::epoch_millis(2026, 11, 9);
        let created = as_of - 60 * 86_400_000;
        let halves = format!(
            r#"{{"logGroups":[
                {{"logGroupName":"/a","storedBytes":1073741824,"creationTime":{created}}},
                {{"logGroupName":"/b","storedBytes":1073741824,"creationTime":{created}}}
            ]}}"#
        );
        let bill = bill_from_inputs(&[inputs(&halves)], &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "0.02");
        assert_eq!(format_usd(bill.rows[1].save_30_usd.unwrap()), "0.02");
        assert_eq!(format_usd(bill.totals.save_30_usd.unwrap()), "0.03");
    }

    #[test]
    fn huge_rate_is_an_input_error() {
        let text = include_str!("../prices/cloudwatch-logs-storage.json").replace(
            "\"standard_storage_per_gb_month\": \"0.03\"",
            "\"standard_storage_per_gb_month\": \"79228162514264337593543950335\"",
        );
        let prices = crate::price::load_prices_str(&text).unwrap();
        let options = Options {
            prices,
            ..Options::default()
        };
        let zero = bill_from_inputs(
            &[inputs(r#"{"logGroupName":"/a","storedBytes":0}"#)],
            &options,
        )
        .unwrap();
        assert_eq!(format_usd(zero.rows[0].usd_month), "0.00");
        let err = bill_from_inputs(
            &[inputs(r#"{"logGroupName":"/a","storedBytes":2147483648}"#)],
            &options,
        )
        .unwrap_err();
        assert_eq!(err.message(), "price overflow");
        assert!(!err.message().contains("79228162514264337593543950335"));
        assert!(!err.message().contains("2147483648"));
    }

    #[test]
    fn summing_two_huge_rows_overflows_the_total() {
        let text = include_str!("../prices/cloudwatch-logs-storage.json").replace(
            "\"standard_storage_per_gb_month\": \"0.03\"",
            "\"standard_storage_per_gb_month\": \"79228162514264337593543950335\"",
        );
        let options = Options {
            prices: crate::price::load_prices_str(&text).unwrap(),
            ..Options::default()
        };
        let one = r#"{"logGroupName":"/a","storedBytes":1073741824}"#;
        assert!(bill_from_inputs(&[inputs(one)], &options).is_ok());
        let two = r#"{"logGroups":[
            {"logGroupName":"/a","storedBytes":1073741824},
            {"logGroupName":"/b","storedBytes":1073741824}
        ]}"#;
        let err = bill_from_inputs(&[inputs(two)], &options).unwrap_err();
        assert_eq!(err.message(), "price overflow");
    }
}
