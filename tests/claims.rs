//! Each numbered README claim has a test whose name starts `claim_c<N>_`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use cwlogs_retention::{
    bill_from_inputs, days_from_civil, epoch_millis, format_dec, format_usd, load_prices_str,
    prices_builtin, render, valid_log_group_name, Bill, Class, NamedBytes, Options, FREE_TIER_LINE,
    MAX_FILE_BYTES, NOT_PRICED_LINE,
};

const GIB: u64 = 1_073_741_824;

fn named(json: &str) -> NamedBytes {
    NamedBytes {
        name: "f.json".to_string(),
        bytes: json.as_bytes().to_vec(),
        strict: true,
    }
}

fn bill_of(json: &str, as_of: Option<&str>, fail: Option<u64>) -> Bill {
    let options = Options {
        as_of: as_of.map(str::to_string),
        fail_stored_bytes: fail,
        ..Options::default()
    };
    bill_from_inputs(&[named(json)], &options).unwrap()
}

fn with_prices(text: &str) -> Options {
    Options {
        prices: load_prices_str(text).unwrap(),
        ..Options::default()
    }
}

fn group(name: &str, stored: u64, extra: &str) -> String {
    format!(r#"{{"logGroupName":"{name}","storedBytes":{stored}{extra}}}"#)
}

fn src_text() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = String::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push_str(&fs::read_to_string(&path).unwrap());
            out.push('\n');
        }
    }
    out
}

#[test]
fn claim_c1_one_object_is_one_group() {
    let one = bill_of(&group("/a", 1, ""), None, None);
    assert_eq!(one.rows.len(), 1);
    let envelope = bill_of(
        &format!(
            r#"{{"logGroups":[{},{}]}}"#,
            group("/a", 1, ""),
            group("/b", 2, "")
        ),
        None,
        None,
    );
    assert_eq!(envelope.rows.len(), 2);
    assert_eq!(envelope.rows[0].name, "/a");
    assert_eq!(envelope.rows[1].name, "/b");
    let array = bill_of(&format!("[{}]", group("/only", 4, "")), None, None);
    assert_eq!(array.rows.len(), 1);
    assert_eq!(array.rows[0].stored_bytes, 4);
}

#[test]
fn claim_c2_gibibyte_and_terabyte() {
    let one = bill_of(&group("/a", GIB, ""), None, None);
    assert_eq!(format_dec(one.rows[0].gb), "1");
    assert_eq!(format_usd(one.rows[0].usd_month), "0.03");
    let one_tb = GIB * 1024;
    let tb = bill_of(&group("/a", one_tb, ""), None, None);
    assert_eq!(format_dec(tb.rows[0].gb), "1024");
    assert_eq!(format_usd(tb.rows[0].usd_month), "30.72");
    let thirty = 30 * 1024_u64.pow(4);
    assert_eq!(thirty, 32_985_348_833_280);
    let page = bill_of(&group("/a", thirty, ""), None, None);
    assert_eq!(format_dec(page.rows[0].gb), "30720");
    assert_eq!(format_usd(page.rows[0].usd_month), "921.60");
}

#[test]
fn claim_c3_standard_rate() {
    let prices = prices_builtin();
    assert_eq!(format_usd(prices.standard_storage_per_gb_month), "0.03");
    assert_eq!(format_usd(prices.rate(Class::Standard)), "0.03");
    let bill = bill_of(
        &group("/a", GIB, r#","logGroupClass":"STANDARD""#),
        None,
        None,
    );
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
}

#[test]
fn claim_c4_infrequent_access_rate_and_override() {
    let prices = prices_builtin();
    assert_eq!(
        format_usd(prices.infrequent_access_storage_per_gb_month),
        "0.03"
    );
    let builtin = bill_of(
        &group("/a", GIB, r#","logGroupClass":"INFREQUENT_ACCESS""#),
        None,
        None,
    );
    assert_eq!(format_usd(builtin.rows[0].usd_month), "0.03");
    let text = include_str!("../prices/cloudwatch-logs-storage.json").replace(
        "\"infrequent_access_storage_per_gb_month\": \"0.03\"",
        "\"infrequent_access_storage_per_gb_month\": \"0.50\"",
    );
    let options = with_prices(&text);
    let bill = bill_from_inputs(
        &[named(&group(
            "/a",
            GIB,
            r#","logGroupClass":"INFREQUENT_ACCESS""#,
        ))],
        &options,
    )
    .unwrap();
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.50");
    assert_eq!(format_usd(options.prices.rate(Class::Standard)), "0.03");
}

#[test]
fn claim_c5_delivery_rate() {
    assert_eq!(
        format_usd(prices_builtin().delivery_storage_per_gb_month),
        "0.03"
    );
    let builtin = bill_of(
        &group("/a", GIB, r#","logGroupClass":"DELIVERY""#),
        None,
        None,
    );
    assert_eq!(format_usd(builtin.rows[0].usd_month), "0.03");
    let text = include_str!("../prices/cloudwatch-logs-storage.json").replace(
        "\"delivery_storage_per_gb_month\": \"0.03\"",
        "\"delivery_storage_per_gb_month\": \"0.07\"",
    );
    let options = with_prices(&text);
    let bill = bill_from_inputs(
        &[named(&group("/a", GIB, r#","logGroupClass":"DELIVERY""#))],
        &options,
    )
    .unwrap();
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.07");
    let standard = bill_from_inputs(
        &[named(&group("/s", GIB, r#","logGroupClass":"STANDARD""#))],
        &options,
    )
    .unwrap();
    assert_eq!(format_usd(standard.rows[0].usd_month), "0.03");
}

#[test]
fn claim_c6_source_date_region() {
    let prices = prices_builtin();
    assert_eq!(prices.source, "https://aws.amazon.com/cloudwatch/pricing/");
    assert_eq!(prices.date, "2026-11-09");
    assert_eq!(prices.region, "us-east-1");
    let bill = bill_of(&group("/a", GIB, ""), None, None);
    let text = render(&bill, false);
    assert!(text.contains(
        "prices: USD, source https://aws.amazon.com/cloudwatch/pricing/, date 2026-11-09, region us-east-1"
    ));
}

#[test]
fn claim_c7_free_tier_not_subtracted() {
    let bill = bill_of(&group("/a", 6 * GIB, ""), None, None);
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.18");
    let text = render(&bill, false);
    assert!(text.contains(FREE_TIER_LINE));
    assert!(text.contains("0.18"));
    let json = render(&bill, true);
    assert!(json.contains("\"free_tier_applied\": false"));
}

#[test]
fn claim_c8_ingestion_not_priced() {
    let file = include_str!("../prices/cloudwatch-logs-storage.json");
    assert!(!file.contains("ingestion"));
    assert!(!file.contains("\"0.50\""));
    assert!(!file.contains("\"0.25\""));
    assert!(!file.contains("\"0.12\""));
    let text = file.replace(
        "\"region\": \"us-east-1\"",
        "\"region\": \"us-east-1\", \"ingestion_per_gb\": \"0.50\"",
    );
    let options = with_prices(&text);
    let bill = bill_from_inputs(&[named(&group("/a", GIB, ""))], &options).unwrap();
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
    let rendered = render(&bill, false);
    assert!(rendered.contains(NOT_PRICED_LINE));
    assert!(!rendered.contains("0.50"));
    assert!(!rendered.contains("0.25"));
    assert!(!rendered.contains("0.12"));
}

#[test]
fn claim_c9_missing_retention_never_expires() {
    let open = bill_of(&group("/a", 10, ""), None, Some(0));
    assert!(open.rows[0].retention_days.is_none());
    assert!(open.threshold_exceeded);
    let text = render(&open, false);
    assert!(text.contains("none"));
    assert!(text.contains("advice /a: no retention"));
    let null_retention = bill_of(
        &group("/a", 10, r#","retentionInDays":null"#),
        None,
        Some(0),
    );
    assert!(null_retention.rows[0].retention_days.is_none());
    assert!(null_retention.threshold_exceeded);
    let kept = bill_of(
        &group("/a", 10_000, r#","retentionInDays":14"#),
        None,
        Some(0),
    );
    assert_eq!(kept.rows[0].retention_days, Some(14));
    assert!(!kept.threshold_exceeded);
    assert!(!render(&kept, false).contains("put-retention-policy"));
}

#[test]
fn claim_c10_steady_ingest_horizon() {
    let as_of = epoch_millis(2026, 11, 9);
    let created = as_of - 100 * 86_400_000;
    let json = group(
        "/a",
        100 * GIB,
        &format!(r#","retentionInDays":50,"creationTime":{created}"#),
    );
    let bill = bill_of(&json, Some("2026-11-09"), None);
    assert_eq!(format_dec(bill.rows[0].age_days.unwrap()), "100");
    assert_eq!(format_usd(bill.rows[0].usd_month), "3.00");
    assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "1.20");
    assert_eq!(format_usd(bill.rows[0].save_90_usd.unwrap()), "0.00");
    let note = render(&bill, false);
    assert!(note.contains("not a forecast"));
    assert!(!note.contains("2.10"));
}

#[test]
fn claim_c11_targets_and_zero_at_horizon() {
    let as_of = epoch_millis(2026, 11, 9);
    let young = group(
        "/a",
        10 * GIB,
        &format!(",\"creationTime\":{}", as_of - 10 * 86_400_000),
    );
    let bill = bill_of(&young, Some("2026-11-09"), None);
    assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "0.00");
    assert_eq!(format_usd(bill.rows[0].save_90_usd.unwrap()), "0.00");
    assert_eq!(format_usd(bill.rows[0].save_365_usd.unwrap()), "0.00");
    let year = group(
        "/a",
        365 * GIB,
        &format!(",\"creationTime\":{}", as_of - 365 * 86_400_000),
    );
    let bill = bill_of(&year, Some("2026-11-09"), None);
    assert_eq!(format_usd(bill.rows[0].save_30_usd.unwrap()), "10.05");
    assert_eq!(format_usd(bill.rows[0].save_90_usd.unwrap()), "8.25");
    assert_eq!(format_usd(bill.rows[0].save_365_usd.unwrap()), "0.00");
}

#[test]
fn claim_c12_as_of_midnight_utc() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(epoch_millis(1970, 1, 1), 0);
    let span = epoch_millis(2026, 11, 9) - epoch_millis(2025, 11, 9);
    assert_eq!(span, 365 * 86_400_000);
    let created = epoch_millis(2026, 11, 9) - 43_200_000;
    let json = group("/a", GIB, &format!(",\"creationTime\":{created}"));
    let bill = bill_of(&json, Some("2026-11-09"), None);
    assert_eq!(format_dec(bill.rows[0].age_days.unwrap()), "0.5");
    let text = render(&bill, false);
    assert!(text.contains("as-of: 2026-11-09"));
}

#[test]
fn claim_c13_savings_omitted_without_as_of_or_creation() {
    let with_creation = group(
        "/a",
        GIB,
        &format!(",\"creationTime\":{}", epoch_millis(2025, 11, 9)),
    );
    let bill = bill_of(&with_creation, None, None);
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
    assert!(bill.rows[0].save_30_usd.is_none());
    assert!(render(&bill, false).contains("as-of: not set"));
    let bill = bill_of(&group("/a", GIB, ""), Some("2026-11-09"), None);
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
    assert!(bill.rows[0].age_days.is_none());
    assert!(bill.rows[0].save_90_usd.is_none());
    assert!(bill.totals.save_365_usd.is_none());
    let text = render(&bill, false);
    assert!(text.contains("as-of: 2026-11-09"));
    assert!(text
        .lines()
        .any(|line| line.contains("/a") && line.contains("0.03")));
}

#[test]
fn claim_c14_fail_stored_bytes_exit_1() {
    let open = bill_of(&group("/a", 10, ""), None, Some(10));
    assert!(!open.threshold_exceeded);
    let over = bill_of(&group("/a", 11, ""), None, Some(10));
    assert!(over.threshold_exceeded);
    assert!(render(&over, false).contains("threshold exceeded: yes"));
    assert!(render(&over, false).contains("prices:"));
    let kept = bill_of(&group("/a", 100, r#","retentionInDays":30"#), None, Some(0));
    assert!(!kept.threshold_exceeded);
    let dir = std::env::temp_dir().join(format!("cwlogs-retention-c14-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("groups.json");
    fs::write(&path, group("/a", 11, "")).unwrap();
    let (code, stdout, stderr) =
        run_bin(&["--fail-stored-bytes=10", "--json", path.to_str().unwrap()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stdout.contains("\"threshold_exceeded\": true"));
    assert!(stdout.contains("\"estimated_usd_month\""));
    assert!(stderr.is_empty());
    let (code, stdout, _) = run_bin(&["--fail-stored-bytes=11", path.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("threshold exceeded: no"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn claim_c15_json_money_strings() {
    let bill = bill_of(&group("/a", GIB, ""), Some("2026-11-09"), None);
    let text = render(&bill, true);
    assert!(text.ends_with('\n'));
    assert!(text.contains("\"version\": \"1.0.1\""));
    assert!(text.contains("\"source\": \"https://aws.amazon.com/cloudwatch/pricing/\""));
    assert!(text.contains("\"date\": \"2026-11-09\""));
    assert!(text.contains("\"estimated_usd_month\": \"0.03\""));
    assert!(!text.contains("\"estimated_usd_month\": 0.03"));
    assert!(text.contains("\"save_30_usd\": null"));
}

#[test]
fn claim_c16_advice_is_not_executed() {
    let as_of = epoch_millis(2026, 11, 9);
    let json = group(
        "/aws/lambda/checkout",
        365 * GIB,
        &format!(",\"creationTime\":{}", as_of - 365 * 86_400_000),
    );
    let bill = bill_of(&json, Some("2026-11-09"), None);
    let text = render(&bill, false);
    let mut saw = 0;
    for line in text.lines() {
        if line.contains("put-retention-policy") {
            assert!(line.contains("Not executed"), "{line}");
            saw += 1;
        }
    }
    assert_eq!(saw, 3);
    let sources = src_text();
    for needle in [
        "Command",
        "std::net",
        "TcpStream",
        "aws-sdk",
        "aws_sdk",
        "reqwest",
        "hyper",
        "tokio",
    ] {
        assert!(!sources.contains(needle), "{needle} is in src");
    }
    let dir = std::env::temp_dir().join(format!("cwlogs-retention-c16-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("groups.json");
    fs::write(&path, &json).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cwlogs-retention"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd.env("CWLOGS_RETENTION_TODAY", "2026-11-20");
    cmd.env("PATH", "/nonexistent-cwlogs-path");
    cmd.args(["--as-of", "2026-11-09", path.to_str().unwrap()]);
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Not executed"));
    assert!(stdout.contains("put-retention-policy"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn claim_c17_no_sdk_and_next_token_not_followed() {
    let cargo =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for needle in ["aws-sdk", "aws_sdk", "reqwest", "hyper", "tokio"] {
        assert!(!cargo.contains(needle), "{needle}");
    }
    assert_eq!(MAX_FILE_BYTES, 268_435_456);
    let json = format!(
        r#"{{"nextToken":"SECRETTOKEN99","logGroups":[{}]}}"#,
        group("/a", 1, "")
    );
    let bill = bill_of(&json, None, None);
    let text = render(&bill, false);
    assert!(text.contains("nextToken was not followed"));
    assert!(!text.contains("SECRETTOKEN99"));
    assert!(!src_text().contains("std::net"));
}

#[test]
fn claim_c18_duplicate_names() {
    let json = format!(
        r#"{{"logGroups":[{},{}]}}"#,
        group("/a", 1, r#","note":"RAWJSONSECRET""#),
        group("/a", 2, "")
    );
    let err = bill_from_inputs(&[named(&json)], &Options::default()).unwrap_err();
    assert_eq!(err.message(), "duplicate log group /a");
    assert!(!err.message().contains("RAWJSONSECRET"));
    assert!(!err.message().contains("logGroups"));
}

#[test]
fn claim_c19_stored_bytes_integer() {
    let ok = bill_of(
        r#"{"logGroupName":"/a","storedBytes":"1073741824"}"#,
        None,
        None,
    );
    assert_eq!(ok.rows[0].stored_bytes, GIB);
    let exact = bill_of(
        r#"{"logGroupName":"/a","storedBytes":9007199254740995}"#,
        None,
        None,
    );
    assert_eq!(exact.rows[0].stored_bytes, 9_007_199_254_740_995);
    assert!(render(&exact, false).contains("9007199254740995"));
    for (body, secret) in [
        (r#"{"logGroupName":"/a","storedBytes":1e10}"#, "1e10"),
        (r#"{"logGroupName":"/a","storedBytes":-5}"#, "-5"),
        (r#"{"logGroupName":"/a","storedBytes":1.5}"#, "1.5"),
    ] {
        let err = bill_from_inputs(&[named(body)], &Options::default()).unwrap_err();
        assert!(
            err.message().contains("bad storedBytes"),
            "{}",
            err.message()
        );
        assert!(!err.message().contains(secret), "{}", err.message());
    }
}

#[test]
fn claim_c20_log_group_name_alphabet() {
    assert!(valid_log_group_name("/aws/lambda/a_b-c.d#e"));
    assert!(valid_log_group_name(&"a".repeat(512)));
    assert!(!valid_log_group_name(&"a".repeat(513)));
    let bill = bill_of(&group("/aws/lambda/a_b-c.d#e", 1, ""), None, None);
    assert_eq!(bill.rows[0].name, "/aws/lambda/a_b-c.d#e");
    let err = bill_from_inputs(
        &[named(r#"{"logGroupName":"SECRET!!","storedBytes":1}"#)],
        &Options::default(),
    )
    .unwrap_err();
    assert!(err.message().contains("bad log group name"));
    assert!(!err.message().contains("SECRET"));
}

#[test]
fn claim_c22_arn_account_hidden() {
    let json = r#"{"logGroupName":"/a","storedBytes":1,"arn":"arn:aws:logs:eu-central-1:999888777666:log-group:/a","kmsKeyId":"SECRETKEY"}"#;
    let bill = bill_of(json, None, None);
    assert_eq!(bill.rows[0].class, Class::Standard);
    let text = render(&bill, false);
    assert!(text.contains("eu-central-1"));
    assert!(text.contains("us-east-1"));
    assert!(!text.contains("999888777666"));
    assert!(!text.contains("SECRETKEY"));
    assert_eq!(
        text.matches("log group region eu-central-1 differs from price region us-east-1")
            .count(),
        1
    );
}

#[test]
fn claim_c23_total_is_the_sum() {
    let json = format!(
        r#"{{"logGroups":[{},{}]}}"#,
        group("/a", GIB, ""),
        group("/b", 2 * GIB, "")
    );
    let bill = bill_of(&json, None, None);
    assert_eq!(format_usd(bill.rows[0].usd_month), "0.03");
    assert_eq!(format_usd(bill.rows[1].usd_month), "0.06");
    assert_eq!(format_usd(bill.totals.usd_month), "0.09");
    assert_eq!(bill.totals.stored_bytes, 3 * GIB);
    assert_eq!(format_dec(bill.totals.gb), "3");
    let text = render(&bill, false);
    assert!(text.contains("total"));
    assert!(!text.contains("per day"));
    assert!(!text.contains("factor:"));
    assert!(!text.contains("0.90"));
}

#[test]
fn claim_c24_errors_hide_file_contents() {
    let err =
        bill_from_inputs(&[named("NOTJSON SECRETFILEBODY")], &Options::default()).unwrap_err();
    assert!(err.message().contains("not JSON"));
    assert!(!err.message().contains("SECRETFILEBODY"));
    assert!(!err.message().contains("NOTJSON"));
    let err = load_prices_str("{\"currency\":\"USD\",\"currency\":\"SECRETTOKEN\"}").unwrap_err();
    assert_eq!(err.message(), "duplicate key");
    assert!(!err.message().contains("SECRETTOKEN"));
}

fn run_bin(args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cwlogs-retention"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd.env("CWLOGS_RETENTION_TODAY", "2026-11-20");
    cmd.args(args);
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn advice_command_quotes_the_name_for_the_shell() {
    let as_of = epoch_millis(2026, 11, 9);
    let extra = format!(",\"creationTime\":{}", as_of - 365 * 86_400_000);
    for name in ["#x", "-x", "/a#b"] {
        let bill = bill_of(&group(name, 365 * GIB, &extra), Some("2026-11-09"), None);
        let text = render(&bill, false);
        let mut saw = 0;
        for line in text.lines().filter(|l| l.contains("put-retention-policy")) {
            assert!(
                line.contains(&format!("--log-group-name='{name}' --retention-in-days ")),
                "{line}"
            );
            saw += 1;
        }
        assert_eq!(saw, 3, "{name}");
    }
}

#[test]
fn unretained_group_without_savings_gets_one_unestimated_advice_line() {
    let bill = bill_of(&group("/a", GIB, ""), None, None);
    let text = render(&bill, false);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("advice /a:"))
        .collect();
    assert_eq!(
        lines,
        [
            "advice /a: no retention; savings not estimated for 30, 90, and 365 days. Not executed: aws logs put-retention-policy --log-group-name='/a' --retention-in-days 30"
        ]
    );
    let json: serde_json::Value = serde_json::from_str(&render(&bill, true)).unwrap();
    assert_eq!(
        json["log_groups"][0]["advice"],
        serde_json::json!([lines[0]])
    );
}

#[test]
fn rows_are_ordered_by_name_not_by_input_order() {
    let json = format!(
        r#"{{"logGroups":[{},{},{}]}}"#,
        group("/c", 1, ""),
        group("/a", 1, ""),
        group("/b", 1, "")
    );
    let bill = bill_of(&json, None, None);
    let names: Vec<&str> = bill.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["/a", "/b", "/c"]);
}

#[test]
fn json_for_a_retained_group_without_as_of_has_nulls_and_no_advice() {
    let bill = bill_of(&group("/a", GIB, r#","retentionInDays":14"#), None, None);
    let json: serde_json::Value = serde_json::from_str(&render(&bill, true)).unwrap();
    let row = &json["log_groups"][0];
    assert_eq!(row["retention_days"], 14);
    assert!(row["age_days"].is_null());
    assert!(row["save_30_usd"].is_null());
    assert!(row["save_90_usd"].is_null());
    assert!(row["save_365_usd"].is_null());
    assert_eq!(row["advice"], serde_json::json!([]));
    assert!(json["totals"]["save_30_usd"].is_null());
}
