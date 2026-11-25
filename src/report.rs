use std::fmt::Write;

use rust_decimal::Decimal;
use serde_json::{json, Value};

use crate::bill::{Bill, Row};
use crate::money::{format_dec, format_gb, format_usd};

pub const FREE_TIER_LINE: &str = "free tier not applied: pricing examples waive the first 5 GB of archived logs per month for the account, and this export is not the whole account";

/// Version of the JSON document layout. It changes only when a key is removed or changes meaning.
pub const SCHEMA_VERSION: u32 = 1;

pub const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

pub const NOT_PRICED_LINE: &str = "not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.";

pub const SAVINGS_NOTE: &str = "savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.";

/// Output format of the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Markdown,
}

pub fn render(bill: &Bill, json_output: bool) -> String {
    render_with(
        bill,
        if json_output {
            Format::Json
        } else {
            Format::Text
        },
        false,
    )
}

/// Like [`render`]; `quiet` keeps only the result in text and Markdown. The JSON is always whole.
pub fn render_with(bill: &Bill, format: Format, quiet: bool) -> String {
    match (format, quiet) {
        (Format::Json, _) => render_json(bill),
        (Format::Text, false) => render_text(bill),
        (Format::Text, true) => render_text_quiet(bill),
        (Format::Markdown, quiet) => render_markdown(bill, quiet),
    }
}

/// One table cell: backslash and pipe escaped, control and direction characters made visible.
pub(crate) fn md_cell(text: &str) -> String {
    visible(&text.replace('\\', "\\\\").replace('|', "\\|"))
}

fn md_row(cells: &[String]) -> String {
    let cells: Vec<String> = cells.iter().map(|cell| md_cell(cell)).collect();
    format!("| {} |\n", cells.join(" | "))
}

fn render_markdown(bill: &Bill, quiet: bool) -> String {
    let mut out = String::new();
    if !quiet {
        out.push_str("# cwlogs-retentie\n\n");
    }
    if let Some(limit) = bill.fail_stored_bytes {
        let yn = if bill.threshold_exceeded { "yes" } else { "no" };
        let _ = writeln!(out, "fail-stored-bytes: {limit}  ");
        let _ = writeln!(out, "threshold exceeded: {yn}\n");
    }
    let _ = writeln!(out, "{ESTIMATE_LINE}\n");
    if !quiet {
        let prices = &bill.prices;
        let _ = writeln!(
            out,
            "prices: {}, source {}, date {}, region {}  ",
            md_cell(&prices.currency),
            md_cell(&prices.source),
            md_cell(&prices.date),
            md_cell(&prices.region)
        );
        let as_of = bill.as_of.as_deref().unwrap_or("not set");
        let _ = writeln!(out, "as-of: {}\n", md_cell(as_of));
        let _ = writeln!(out, "{FREE_TIER_LINE}  ");
        let _ = writeln!(out, "{NOT_PRICED_LINE}\n");
        for warning in &bill.warnings {
            let _ = writeln!(out, "> warning: {}\n", md_cell(warning));
        }
    }
    let rows = table_rows(bill);
    out.push_str(&md_row(&rows[0]));
    let right = [false, false, true, true, true, true, true, true, true, true];
    let separator: Vec<String> = right
        .iter()
        .map(|r| if *r { "---:" } else { "---" }.to_string())
        .collect();
    out.push_str(&format!("| {} |\n", separator.join(" | ")));
    for row in &rows[1..] {
        out.push_str(&md_row(row));
    }
    if !quiet {
        out.push('\n');
        for row in &bill.rows {
            for line in advice_lines(row) {
                let _ = writeln!(out, "- {}", md_cell(&line));
            }
        }
        let _ = writeln!(out, "\nnote: {SAVINGS_NOTE}");
    }
    out
}

fn render_text_quiet(bill: &Bill) -> String {
    let mut out = String::new();
    if let Some(limit) = bill.fail_stored_bytes {
        let yn = if bill.threshold_exceeded { "yes" } else { "no" };
        let _ = writeln!(out, "fail-stored-bytes: {limit}");
        let _ = writeln!(out, "threshold exceeded: {yn}");
    }
    let _ = writeln!(out, "{ESTIMATE_LINE}");
    out.push('\n');
    push_table(&mut out, &table_rows(bill));
    out
}

/// Make invisible, line-breaking and direction-changing characters visible.
pub(crate) fn visible(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if crate::error::must_escape(ch) {
            out.push_str(&format!("\\u{{{:04x}}}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

fn render_text(bill: &Bill) -> String {
    let mut out = String::new();
    for warning in &bill.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
    let prices = &bill.prices;
    let _ = writeln!(
        out,
        "prices: {}, source {}, date {}, region {}",
        prices.currency,
        visible(&prices.source),
        prices.date,
        prices.region
    );
    match &bill.as_of {
        Some(date) => {
            let _ = writeln!(out, "as-of: {date}");
        }
        None => {
            let _ = writeln!(out, "as-of: not set");
        }
    }
    if let Some(limit) = bill.fail_stored_bytes {
        let yn = if bill.threshold_exceeded { "yes" } else { "no" };
        let _ = writeln!(out, "fail-stored-bytes: {limit}");
        let _ = writeln!(out, "threshold exceeded: {yn}");
    }
    let _ = writeln!(out, "{FREE_TIER_LINE}");
    let _ = writeln!(out, "{NOT_PRICED_LINE}");
    let _ = writeln!(out, "{ESTIMATE_LINE}");
    out.push('\n');
    push_table(&mut out, &table_rows(bill));
    out.push('\n');
    for row in &bill.rows {
        for line in advice_lines(row) {
            let _ = writeln!(out, "{line}");
        }
    }
    let _ = writeln!(out, "note: {SAVINGS_NOTE}");
    out
}

fn table_rows(bill: &Bill) -> Vec<Vec<String>> {
    let header = [
        "log_group",
        "class",
        "retention",
        "age_days",
        "stored_bytes",
        "gb",
        "estimated_usd_month",
        "save_30_usd",
        "save_90_usd",
        "save_365_usd",
    ]
    .map(str::to_string);
    let mut table = vec![header.to_vec()];
    for row in &bill.rows {
        table.push(vec![
            row.name.clone(),
            row.class.to_string(),
            retention_cell(row.retention_days),
            dec_cell(row.age_days),
            row.stored_bytes.to_string(),
            format_gb(row.gb),
            format_usd(row.usd_month),
            usd_cell(row.save_30_usd),
            usd_cell(row.save_90_usd),
            usd_cell(row.save_365_usd),
        ]);
    }
    let totals = &bill.totals;
    table.push(vec![
        "total".to_string(),
        String::new(),
        String::new(),
        String::new(),
        totals.stored_bytes.to_string(),
        format_gb(totals.gb),
        format_usd(totals.usd_month),
        usd_cell(totals.save_30_usd),
        usd_cell(totals.save_90_usd),
        usd_cell(totals.save_365_usd),
    ]);
    table
}

fn retention_cell(days: Option<u32>) -> String {
    match days {
        Some(days) => days.to_string(),
        None => "none".to_string(),
    }
}

fn dec_cell(value: Option<Decimal>) -> String {
    value.map(format_age).unwrap_or_default()
}

/// Age for display: at most 6 decimals, half away from zero. Billing keeps the full value.
fn format_age(value: Decimal) -> String {
    format_dec(
        value.round_dp_with_strategy(6, rust_decimal::RoundingStrategy::MidpointAwayFromZero),
    )
}

fn usd_cell(value: Option<Decimal>) -> String {
    value.map(format_usd).unwrap_or_default()
}

fn push_table(out: &mut String, rows: &[Vec<String>]) {
    let mut widths = vec![0; rows[0].len()];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    let right = [false, false, true, true, true, true, true, true, true, true];
    for row in rows {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            if right[index] {
                let _ = write!(line, "{:>width$}", cell, width = widths[index]);
            } else {
                let _ = write!(line, "{:<width$}", cell, width = widths[index]);
            }
        }
        out.push_str(line.trim_end_matches(|c: char| c.is_ascii_whitespace()));
        out.push('\n');
    }
}

fn advice_lines(row: &Row) -> Vec<String> {
    if row.retention_days.is_some() {
        return Vec::new();
    }
    match (row.save_30_usd, row.save_90_usd, row.save_365_usd) {
        (Some(save_30), Some(save_90), Some(save_365)) => vec![
            advice_line(&row.name, 30, save_30),
            advice_line(&row.name, 90, save_90),
            advice_line(&row.name, 365, save_365),
        ],
        _ => vec![format!(
            "advice {name}: no retention; savings not estimated for 30, 90, and 365 days. Not executed: aws logs put-retention-policy --log-group-name='{name}' --retention-in-days 30",
            name = row.name
        )],
    }
}

fn advice_line(name: &str, days: u32, usd: Decimal) -> String {
    format!(
        "advice {name}: no retention; {days} days saves {usd} USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='{name}' --retention-in-days {days}",
        usd = format_usd(usd)
    )
}

fn render_json(bill: &Bill) -> String {
    let prices = &bill.prices;
    let groups: Vec<Value> = bill
        .rows
        .iter()
        .map(|row| {
            json!({
                "log_group": row.name,
                "class": row.class.as_str(),
                "retention_days": row.retention_days,
                "age_days": dec_json(row.age_days),
                "stored_bytes": row.stored_bytes,
                "gb": format_gb(row.gb),
                "estimated_usd_month": format_usd(row.usd_month),
                "save_30_usd": usd_json(row.save_30_usd),
                "save_90_usd": usd_json(row.save_90_usd),
                "save_365_usd": usd_json(row.save_365_usd),
                "advice": advice_lines(row),
            })
        })
        .collect();
    let totals = &bill.totals;
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "currency": prices.currency,
        "source": visible(&prices.source),
        "date": prices.date,
        "region": prices.region,
        "as_of": bill.as_of,
        "estimate": true,
        "estimate_note": ESTIMATE_LINE,
        "free_tier_applied": false,
        "not_priced": NOT_PRICED_LINE,
        "fail_stored_bytes": bill.fail_stored_bytes,
        "threshold_exceeded": bill.threshold_exceeded,
        "note": SAVINGS_NOTE,
        "warnings": bill.warnings,
        "log_groups": groups,
        "totals": {
            "stored_bytes": totals.stored_bytes,
            "gb": format_gb(totals.gb),
            "estimated_usd_month": format_usd(totals.usd_month),
            "save_30_usd": usd_json(totals.save_30_usd),
            "save_90_usd": usd_json(totals.save_90_usd),
            "save_365_usd": usd_json(totals.save_365_usd),
        }
    });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

fn dec_json(value: Option<Decimal>) -> Value {
    match value {
        Some(value) => Value::String(format_age(value)),
        None => Value::Null,
    }
}

fn usd_json(value: Option<Decimal>) -> Value {
    match value {
        Some(value) => Value::String(format_usd(value)),
        None => Value::Null,
    }
}

#[cfg(test)]
mod markdown_tests {
    use super::md_cell;

    #[test]
    fn a_cell_cannot_break_the_table() {
        assert_eq!(md_cell("a|b"), "a\\|b");
        assert_eq!(md_cell("a\nb\rc"), "a\\u{000a}b\\u{000d}c");
        assert_eq!(md_cell("a\\|b"), "a\\\\\\|b");
        assert_eq!(md_cell("plain"), "plain");
    }
}

#[cfg(test)]
mod width_tests {
    use super::push_table;

    #[test]
    fn table_width_counts_chars_not_bytes() {
        let rows = vec![
            vec!["name".to_string(), "x".to_string()],
            vec!["é".to_string(), "y".to_string()],
            vec!["abc".to_string(), "z".to_string()],
        ];
        let mut out = String::new();
        push_table(&mut out, &rows);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, ["name  x", "é     y", "abc   z"]);
    }

    #[test]
    fn table_line_strips_only_ascii_trailing_whitespace() {
        let rows = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d\u{a0}".to_string()],
        ];
        let mut out = String::new();
        push_table(&mut out, &rows);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, ["a  b", "c  d\u{a0}"]);
    }
}
