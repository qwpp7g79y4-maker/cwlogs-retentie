use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::class::Class;
use crate::error::Error;
use crate::json_scan::{scan_json, KeyFault};
use crate::money::{parse_money, valid_date};

/// Archived-storage rates for one quote. Ingestion is not a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prices {
    pub currency: String,
    pub source: String,
    pub date: String,
    pub region: String,
    pub standard_storage_per_gb_month: Decimal,
    pub infrequent_access_storage_per_gb_month: Decimal,
    pub delivery_storage_per_gb_month: Decimal,
}

impl Prices {
    #[must_use]
    pub fn rate(&self, class: Class) -> Decimal {
        match class {
            Class::Standard => self.standard_storage_per_gb_month,
            Class::InfrequentAccess => self.infrequent_access_storage_per_gb_month,
            Class::Delivery => self.delivery_storage_per_gb_month,
        }
    }
}

const BUILTIN_JSON: &str = include_str!("../prices/cloudwatch-logs-storage.json");

/// Storage rates from the AWS CloudWatch pricing page, read 2026-11-09.
///
/// The worked examples for US East (N. Virginia) price archived logs at
/// 0.03 USD per GB-month, and they convert 1 TB as 1,024 GB. The Infrequent
/// Access archive example uses the same 0.03 USD figure. The 5 GB waiver in
/// those examples is an account free tier and is not applied here.
#[must_use]
pub fn prices_builtin() -> Prices {
    load_prices_str(BUILTIN_JSON).expect("built-in prices")
}

pub fn load_prices_str(text: &str) -> Result<Prices, Error> {
    let text = strip_bom(text);
    match scan_json(text) {
        Ok(()) => {}
        Err(KeyFault::Duplicate) => return Err(Error::new("duplicate key")),
        Err(KeyFault::Bad) => return Err(Error::new("not JSON")),
    }
    let value: Value = serde_json::from_str(text).map_err(|_| Error::new("not JSON"))?;
    let Some(obj) = value.as_object() else {
        return Err(Error::new("expected an object"));
    };
    Ok(Prices {
        currency: req_currency(obj)?,
        source: req_source(obj)?,
        date: req_date(obj)?,
        region: req_label(obj, "region")?,
        standard_storage_per_gb_month: req_decimal(obj, "standard_storage_per_gb_month")?,
        infrequent_access_storage_per_gb_month: req_decimal(
            obj,
            "infrequent_access_storage_per_gb_month",
        )?,
        delivery_storage_per_gb_month: req_decimal(obj, "delivery_storage_per_gb_month")?,
    })
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn req_currency(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "currency")?;
    if text.len() == 3 && text.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(text)
    } else {
        Err(Error::new("bad currency"))
    }
}

fn req_source(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "source")?;
    if (1..=500).contains(&text.chars().count()) && !text.chars().any(|c| c.is_control()) {
        Ok(text)
    } else {
        Err(Error::new("bad source"))
    }
}

fn req_date(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "date")?;
    if valid_date(&text) {
        Ok(text)
    } else {
        Err(Error::new("bad date"))
    }
}

fn req_label(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let text = req_string(obj, key)?;
    let ok = (1..=64).contains(&text.len())
        && text.chars().all(|c| c.is_ascii_graphic() || c == ' ')
        && !text.chars().any(|c| c.is_control());
    if ok {
        Ok(text)
    } else {
        Err(Error::new(format!("bad {key}")))
    }
}

fn req_decimal(obj: &Map<String, Value>, key: &str) -> Result<Decimal, Error> {
    let Some(value) = obj.get(key) else {
        return Err(Error::new(format!("missing {key}")));
    };
    let raw = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Err(Error::new(format!("bad {key}"))),
    };
    parse_money(&raw).map_err(|()| Error::new(format!("bad {key}")))
}

fn req_string(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let Some(value) = obj.get(key) else {
        return Err(Error::new(format!("missing {key}")));
    };
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::new(format!("bad {key}")))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{load_prices_str, prices_builtin};
    use crate::class::Class;

    fn builtin_text() -> &'static str {
        include_str!("../prices/cloudwatch-logs-storage.json")
    }

    fn rate() -> Decimal {
        "0.03".parse().unwrap()
    }

    #[test]
    fn builtin_matches_the_pricing_page() {
        let prices = prices_builtin();
        assert_eq!(prices.currency, "USD");
        assert_eq!(prices.source, "https://aws.amazon.com/cloudwatch/pricing/");
        assert_eq!(prices.date, "2026-11-09");
        assert_eq!(prices.region, "us-east-1");
        assert_eq!(prices.standard_storage_per_gb_month, rate());
        assert_eq!(prices.infrequent_access_storage_per_gb_month, rate());
        assert_eq!(prices.delivery_storage_per_gb_month, rate());
        assert_eq!(prices.rate(Class::Standard), rate());
        assert_eq!(prices.rate(Class::InfrequentAccess), rate());
        assert_eq!(prices.rate(Class::Delivery), rate());
        assert!(!builtin_text().contains("ingestion"));
        assert!(!builtin_text().contains("0.50"));
        assert!(!builtin_text().contains("0.25"));
        assert!(!builtin_text().contains("0.12"));
    }

    #[test]
    fn unknown_field_is_ignored_and_class_rates_are_independent() {
        let mut text = builtin_text().trim_end().trim_end_matches('}').to_string();
        text.push_str(", \"ingestion_per_gb\": \"0.50\"\n}\n");
        let prices = load_prices_str(&text).unwrap();
        assert_eq!(prices.standard_storage_per_gb_month, rate());
        let ia = builtin_text().replace(
            "\"infrequent_access_storage_per_gb_month\": \"0.03\"",
            "\"infrequent_access_storage_per_gb_month\": \"0.50\"",
        );
        let prices = load_prices_str(&ia).unwrap();
        assert_eq!(
            prices.rate(Class::InfrequentAccess),
            "0.50".parse().unwrap()
        );
        assert_eq!(prices.rate(Class::Standard), rate());
        assert_eq!(prices.rate(Class::Delivery), rate());
    }

    #[test]
    fn unicode_whitespace_around_a_price_string_is_rejected() {
        let ascii = builtin_text().replace(
            "\"standard_storage_per_gb_month\": \"0.03\"",
            "\"standard_storage_per_gb_month\": \" \\t\\r\\n0.03 \"",
        );
        assert_eq!(
            load_prices_str(&ascii)
                .unwrap()
                .standard_storage_per_gb_month,
            rate()
        );
        for ws in ["\\u00a0", "\\u2028", "\\u3000"] {
            for value in [format!("{ws}0.03"), format!("0.03{ws}")] {
                let text = builtin_text().replace(
                    "\"standard_storage_per_gb_month\": \"0.03\"",
                    &format!("\"standard_storage_per_gb_month\": \"{value}\""),
                );
                assert_ne!(text, builtin_text());
                let err = load_prices_str(&text).unwrap_err();
                assert_eq!(err.message(), "bad standard_storage_per_gb_month");
            }
        }
    }

    #[test]
    fn json_number_is_accepted_and_exponent_is_not() {
        let text = r#"{"currency":"USD","source":"https://example.test","date":"2026-11-09","region":"us-east-1","standard_storage_per_gb_month":0.03,"infrequent_access_storage_per_gb_month":"0.03","delivery_storage_per_gb_month":"0.03"}"#;
        assert_eq!(
            load_prices_str(text).unwrap().standard_storage_per_gb_month,
            rate()
        );
        let bad = text.replace("0.03", "1e-7");
        let err = load_prices_str(&bad).unwrap_err();
        assert_eq!(err.message(), "bad standard_storage_per_gb_month");
        assert!(!err.message().contains("1e-7"));
    }

    #[test]
    fn rejects_negative_bad_date_missing_rate_and_lowercase_currency() {
        let base = builtin_text();
        let neg = base.replace("\"0.03\"", "\"-0.03\"");
        let err = load_prices_str(&neg).unwrap_err();
        assert_eq!(err.message(), "bad standard_storage_per_gb_month");
        assert!(!err.message().contains("-0.03"));
        let date = base.replace("2026-11-09", "2025-02-29");
        assert_eq!(load_prices_str(&date).unwrap_err().message(), "bad date");
        let missing = base.replace(
            "\"delivery_storage_per_gb_month\": \"0.03\"",
            "\"nope\": \"1\"",
        );
        assert_eq!(
            load_prices_str(&missing).unwrap_err().message(),
            "missing delivery_storage_per_gb_month"
        );
        let currency = base.replace("\"USD\"", "\"usd\"");
        assert_eq!(
            load_prices_str(&currency).unwrap_err().message(),
            "bad currency"
        );
        assert_eq!(
            load_prices_str("[]").unwrap_err().message(),
            "expected an object"
        );
        assert_eq!(load_prices_str("{").unwrap_err().message(), "not JSON");
    }

    #[test]
    fn duplicate_key_and_secret_are_not_echoed() {
        let text = "{\"currency\":\"USD\",\"currency\":\"SECRETTOKEN\"}";
        let err = load_prices_str(text).unwrap_err();
        assert_eq!(err.message(), "duplicate key");
        assert!(!err.message().contains("SECRETTOKEN"));
        let nested = builtin_text().replace(
            "\"region\": \"us-east-1\"",
            "\"region\": \"us-east-1\", \"extra\": {\"a\": 1, \"a\": 2}",
        );
        assert!(load_prices_str(&nested).is_ok());
    }

    #[test]
    fn bidi_and_line_separators_in_source_are_not_raw_in_output() {
        let bad = builtin_text().replace(
            "https://aws.amazon.com/cloudwatch/pricing/",
            "x\u{202e}y\u{2028}z\u{200f}\u{2066}w",
        );
        let prices = load_prices_str(&bad).unwrap();
        let shown = crate::report::visible(&prices.source);
        assert_eq!(shown, "x\\u{202e}y\\u{2028}z\\u{200f}\\u{2066}w");
        assert!(!shown.contains(['\u{202e}', '\u{2028}', '\u{200f}', '\u{2066}']));
    }

    #[test]
    fn bom_and_control_character_in_source() {
        let text = format!("\u{feff}{}", builtin_text());
        assert_eq!(load_prices_str(&text).unwrap().date, "2026-11-09");
        let bad = builtin_text().replace(
            "https://aws.amazon.com/cloudwatch/pricing/",
            "https://aws.amazon.com/cloudwatch/pricing/\\nSECRET",
        );
        let err = load_prices_str(&bad).unwrap_err();
        assert_eq!(err.message(), "bad source");
        assert!(!err.message().contains("SECRET"));
    }

    #[test]
    fn zero_rate_is_allowed() {
        let text = builtin_text().replace(
            "\"standard_storage_per_gb_month\": \"0.03\"",
            "\"standard_storage_per_gb_month\": \"0\"",
        );
        assert_eq!(
            load_prices_str(&text)
                .unwrap()
                .standard_storage_per_gb_month,
            Decimal::ZERO
        );
    }

    #[test]
    fn every_required_field_reports_missing_and_bad() {
        let base = builtin_text();
        let object: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(base).unwrap();
        for key in [
            "currency",
            "source",
            "date",
            "region",
            "standard_storage_per_gb_month",
            "infrequent_access_storage_per_gb_month",
            "delivery_storage_per_gb_month",
        ] {
            let mut gone = object.clone();
            gone.remove(key);
            let text = serde_json::to_string(&gone).unwrap();
            assert_eq!(
                load_prices_str(&text).unwrap_err().message(),
                format!("missing {key}")
            );
            let mut wrong = object.clone();
            wrong.insert(key.to_string(), serde_json::Value::Bool(true));
            let text = serde_json::to_string(&wrong).unwrap();
            assert_eq!(
                load_prices_str(&text).unwrap_err().message(),
                format!("bad {key}")
            );
        }
        let region = base.replace("\"us-east-1\"", "\"\"");
        assert_eq!(
            load_prices_str(&region).unwrap_err().message(),
            "bad region"
        );
        let region = base.replace("\"us-east-1\"", "\"us\\u00e9\"");
        assert_eq!(
            load_prices_str(&region).unwrap_err().message(),
            "bad region"
        );
        for key in [
            "infrequent_access_storage_per_gb_month",
            "delivery_storage_per_gb_month",
        ] {
            let text = base.replace(
                &format!("\"{key}\": \"0.03\""),
                &format!("\"{key}\": \"-1\""),
            );
            assert_ne!(text, base);
            assert_eq!(
                load_prices_str(&text).unwrap_err().message(),
                format!("bad {key}")
            );
        }
    }
}
