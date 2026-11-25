use std::io::Read;

use flate2::bufread::GzDecoder;
use serde_json::{Map, Value};

use crate::class::Class;
use crate::error::{escape_controls, Error};
use crate::json_scan::{scan_json, KeyFault};
use crate::MAX_FILE_BYTES;

/// One log group from a describe-log-groups document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub class: Class,
    pub retention_days: Option<u32>,
    pub stored_bytes: u64,
    pub creation_ms: Option<i64>,
    pub region: Option<String>,
}

/// Groups and warnings from one file. Warnings never include secrets or tokens.
/// `skipped` holds the warning of a directory member that was left out of the total
/// (also present in `warnings`).
#[derive(Debug)]
pub struct Document {
    pub groups: Vec<Group>,
    pub warnings: Vec<String>,
    pub skipped: Vec<String>,
}

/// CloudWatch's selectable retention values, in days. Never Expire is absence.
#[must_use]
pub fn is_cloudwatch_retention(days: u32) -> bool {
    matches!(
        days,
        1 | 3
            | 5
            | 7
            | 14
            | 30
            | 60
            | 90
            | 120
            | 150
            | 180
            | 365
            | 400
            | 545
            | 731
            | 1096
            | 1827
            | 2192
            | 2557
            | 2922
            | 3288
            | 3653
    )
}

#[must_use]
pub fn valid_log_group_name(name: &str) -> bool {
    (1..=512).contains(&name.len())
        && name.bytes().all(|byte| {
            matches!(
                byte,
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'/' | b'.' | b'#'
            )
        })
}

pub fn parse_document(name: &str, bytes: &[u8], strict: bool) -> Result<Document, Error> {
    let name = escape_controls(name);
    let plain = decompress(&name, bytes)?;
    let text = match std::str::from_utf8(&plain) {
        Ok(text) => text,
        Err(_) if !strict => {
            return Ok(skipped_document(
                format!("skipped {name}: not UTF-8"),
                Vec::new(),
            ));
        }
        Err(_) => return Err(Error::new(format!("{name}: not UTF-8"))),
    };
    let text = strip_bom(text);
    if text.bytes().all(|byte| byte.is_ascii_whitespace()) {
        return empty_document(&name, strict, Vec::new());
    }
    match scan_json(text) {
        Ok(()) => {}
        Err(KeyFault::Duplicate) => return Err(Error::new(format!("{name}: duplicate key"))),
        Err(KeyFault::Bad) => return not_json(&name, strict, text),
    }
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return not_json(&name, strict, text);
    };
    let (groups, warnings) = interpret(&name, &value)?;
    if groups.is_empty() {
        return empty_document(&name, strict, warnings);
    }
    Ok(Document {
        groups,
        warnings,
        skipped: Vec::new(),
    })
}

fn skipped_document(line: String, mut warnings: Vec<String>) -> Document {
    warnings.push(line.clone());
    Document {
        groups: Vec::new(),
        warnings,
        skipped: vec![line],
    }
}

fn not_json(name: &str, strict: bool, text: &str) -> Result<Document, Error> {
    let looks_like_json = text
        .trim_start()
        .as_bytes()
        .first()
        .is_some_and(|byte| matches!(byte, b'{' | b'['));
    if strict || looks_like_json {
        return Err(Error::new(format!("{name}: not JSON")));
    }
    Ok(skipped_document(
        format!("skipped {name}: not JSON"),
        Vec::new(),
    ))
}

fn empty_document(name: &str, strict: bool, warnings: Vec<String>) -> Result<Document, Error> {
    if strict {
        return Err(Error::new(format!("{name}: no log groups")));
    }
    Ok(skipped_document(
        format!("skipped {name}: no log groups"),
        warnings,
    ))
}

fn decompress(name: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut rest = bytes;
        let mut out = Vec::new();
        GzDecoder::new(&mut rest)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut out)
            .map_err(|_| Error::new(format!("{name}: could not read gzip")))?;
        if out.len() as u64 > MAX_FILE_BYTES {
            return Err(Error::new(format!(
                "{name}: file is larger than 268435456 bytes"
            )));
        }
        if !rest.is_empty() {
            return Err(Error::new(format!(
                "{name}: bytes after the first gzip member"
            )));
        }
        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn interpret(file: &str, value: &Value) -> Result<(Vec<Group>, Vec<String>), Error> {
    match value {
        Value::Array(items) => {
            let mut warnings = Vec::new();
            let groups = groups_from_array(file, items, &mut warnings)?;
            Ok((groups, warnings))
        }
        Value::Object(map) if map.contains_key("logGroups") => {
            if map.contains_key("logGroupName") {
                return Err(Error::new(format!(
                    "{file}: both logGroups and logGroupName"
                )));
            }
            let mut warnings = Vec::new();
            note_next_token(file, map, &mut warnings)?;
            let Some(Value::Array(items)) = map.get("logGroups") else {
                return Err(Error::new(format!("{file}: bad logGroups")));
            };
            let groups = groups_from_array(file, items, &mut warnings)?;
            Ok((groups, warnings))
        }
        Value::Object(map) if map.contains_key("logGroupName") => {
            let mut warnings = Vec::new();
            let group = one_group(file, map, &mut warnings)?;
            Ok((vec![group], warnings))
        }
        _ => Err(Error::new(format!("{file}: expected log groups"))),
    }
}

fn note_next_token(
    file: &str,
    map: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    let Some(token) = map.get("nextToken") else {
        return Ok(());
    };
    match token {
        Value::Null => Ok(()),
        Value::String(text) if text.is_empty() => Ok(()),
        Value::String(_) => {
            warnings.push(format!(
                "{file}: describe-log-groups page may be partial; nextToken was not followed"
            ));
            Ok(())
        }
        _ => Err(Error::new(format!("{file}: bad nextToken"))),
    }
}

fn groups_from_array(
    file: &str,
    items: &[Value],
    warnings: &mut Vec<String>,
) -> Result<Vec<Group>, Error> {
    let mut groups = Vec::with_capacity(items.len());
    for item in items {
        let Some(obj) = item.as_object() else {
            return Err(Error::new(format!("{file}: expected log groups")));
        };
        groups.push(one_group(file, obj, warnings)?);
    }
    Ok(groups)
}

fn one_group(
    file: &str,
    obj: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Result<Group, Error> {
    let name = match obj.get("logGroupName") {
        None => return Err(Error::new(format!("{file}: missing logGroupName"))),
        Some(Value::String(text)) if valid_log_group_name(text) => text.clone(),
        Some(_) => return Err(Error::new(format!("{file}: bad log group name"))),
    };
    let stored_bytes = match obj.get("storedBytes") {
        None => return Err(Error::new(format!("{file}: {name}: missing storedBytes"))),
        Some(value) => {
            json_u64(value).map_err(|()| Error::new(format!("{file}: {name}: bad storedBytes")))?
        }
    };
    let retention_days = match obj.get("retentionInDays") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let days = parse_retention(value)
                .map_err(|()| Error::new(format!("{file}: {name}: bad retentionInDays")))?;
            if !is_cloudwatch_retention(days) {
                warnings.push(format!(
                    "{name}: retentionInDays {days} is not a CloudWatch retention value"
                ));
            }
            Some(days)
        }
    };
    let creation_ms = match obj.get("creationTime") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let n = json_u64(value)
                .map_err(|()| Error::new(format!("{file}: {name}: bad creationTime")))?;
            let ms = i64::try_from(n)
                .map_err(|_| Error::new(format!("{file}: {name}: bad creationTime")))?;
            Some(ms)
        }
    };
    let class = match obj.get("logGroupClass") {
        None | Some(Value::Null) => Class::Standard,
        Some(Value::String(text)) => Class::parse(text)
            .ok_or_else(|| Error::new(format!("{file}: {name}: bad log group class")))?,
        Some(_) => return Err(Error::new(format!("{file}: {name}: bad log group class"))),
    };
    let region_of = |value: Option<&Value>| -> Result<Option<String>, Error> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(arn_region(text)),
            Some(_) => Err(Error::new(format!("{file}: {name}: bad arn"))),
        }
    };
    // A real response holds both `arn` and `logGroupArn`; they name one group and agree on the region.
    let region = match (
        region_of(obj.get("arn"))?,
        region_of(obj.get("logGroupArn"))?,
    ) {
        (Some(a), Some(b)) if a != b => {
            return Err(Error::new(format!(
                "{file}: {name}: both arn and logGroupArn"
            )));
        }
        (Some(region), _) | (None, Some(region)) => Some(region),
        (None, None) => None,
    };
    Ok(Group {
        name,
        class,
        retention_days,
        stored_bytes,
        creation_ms,
        region,
    })
}

fn parse_retention(value: &Value) -> Result<u32, ()> {
    let days = u32::try_from(json_u64(value)?).map_err(|_| ())?;
    if (1..=3653).contains(&days) {
        Ok(days)
    } else {
        Err(())
    }
}

fn json_u64(value: &Value) -> Result<u64, ()> {
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text
            .trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
            .to_string(),
        _ => return Err(()),
    };
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

/// Region from `arn:aws:logs:region:account:...`. The account id is not returned.
fn arn_region(arn: &str) -> Option<String> {
    let mut parts = arn.split(':');
    let partition = parts.next()?;
    let _service_prefix = parts.next()?;
    let service = parts.next()?;
    let region = parts.next()?;
    if partition != "arn" || service != "logs" || region.is_empty() {
        return None;
    }
    let ok = region.len() <= 64
        && region
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        Some(region.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_document, valid_log_group_name};
    use crate::class::Class;

    fn doc(json: &str) -> super::Document {
        parse_document("f.json", json.as_bytes(), true).unwrap()
    }

    #[test]
    fn envelope_array_and_single_object() {
        let envelope = doc(
            r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1},{"logGroupName":"/b","storedBytes":"2","retentionInDays":null}],"nextToken":""}"#,
        );
        assert_eq!(envelope.groups.len(), 2);
        assert!(envelope.warnings.is_empty());
        assert_eq!(envelope.groups[0].stored_bytes, 1);
        assert_eq!(envelope.groups[1].stored_bytes, 2);
        assert!(envelope.groups[1].retention_days.is_none());
        let array = doc(r#"[{"logGroupName":"/a","storedBytes":3}]"#);
        assert_eq!(array.groups[0].stored_bytes, 3);
        let one = doc(r#"{"logGroupName":"/a","storedBytes":4,"logGroupClass":"DELIVERY"}"#);
        assert_eq!(one.groups[0].class, Class::Delivery);
        assert_eq!(one.groups[0].class, Class::parse("DELIVERY").unwrap());
    }

    #[test]
    fn missing_class_is_standard_and_unknown_fields_are_ignored() {
        let parsed = doc(
            r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1,"metricFilterCount":4,"kmsKeyId":"SECRETKEY"}]}"#,
        );
        assert_eq!(parsed.groups[0].class, Class::Standard);
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn next_token_is_not_stored() {
        let parsed = doc(
            r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1}],"nextToken":"SECRETTOKEN99"}"#,
        );
        let warning = parsed.warnings.join("\n");
        assert!(warning.contains("nextToken was not followed"));
        assert!(!warning.contains("SECRETTOKEN99"));
        let err =
            parse_document("f.json", br#"{"logGroups":[],"nextToken":12345}"#, true).unwrap_err();
        assert_eq!(err.message(), "f.json: bad nextToken");
        assert!(!err.message().contains("12345"));
    }

    #[test]
    fn log_groups_that_is_not_an_array_is_rejected() {
        let err = parse_document("f.json", br#"{"logGroups":"SECRETVALUE"}"#, true).unwrap_err();
        assert_eq!(err.message(), "f.json: bad logGroups");
        assert!(!err.message().contains("SECRETVALUE"));
    }

    #[test]
    fn bad_name_and_bad_stored_bytes_are_not_echoed() {
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"SECRET NAME","storedBytes":1}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: bad log group name");
        assert!(!err.message().contains("SECRET"));
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a\u202e\u2028","storedBytes":1}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: bad log group name");
        assert!(!err.message().contains('\u{202e}'));
        assert!(!err.message().contains('\u{2028}'));
        let err = parse_document("f.json", br#"{"logGroupName":"/a","storedBytes":-5}"#, true)
            .unwrap_err();
        assert_eq!(err.message(), "f.json: /a: bad storedBytes");
        assert!(!err.message().contains("-5"));
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1e10}"#,
            true,
        )
        .unwrap_err();
        assert!(err.message().contains("bad storedBytes"));
        assert!(!err.message().contains("1e10"));
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1.5}"#,
            true,
        )
        .unwrap_err();
        assert!(err.message().contains("bad storedBytes"));
        assert!(!err.message().contains("1.5"));
    }

    #[test]
    fn retention_rules() {
        let odd = doc(r#"{"logGroupName":"/a","storedBytes":1,"retentionInDays":2}"#);
        assert_eq!(odd.groups[0].retention_days, Some(2));
        assert!(odd.warnings[0].contains("not a CloudWatch retention value"));
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1,"retentionInDays":0}"#,
            true,
        )
        .unwrap_err();
        assert!(err.message().contains("bad retentionInDays"));
        assert!(!err.message().contains("3654"));
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1,"retentionInDays":3654}"#,
            true,
        )
        .unwrap_err();
        assert!(err.message().contains("bad retentionInDays"));
        assert!(!err.message().contains("3654"));
        let kept = doc(r#"{"logGroupName":"/a","storedBytes":1,"retentionInDays":3653}"#);
        assert!(kept.warnings.is_empty());
        assert_eq!(kept.groups[0].retention_days, Some(3653));
    }

    #[test]
    fn duplicate_top_level_key_hides_the_secret() {
        let err = parse_document(
            "f.json",
            br#"{"logGroups":[],"logGroups":[{"logGroupName":"/a","storedBytes":1,"note":"RAWJSONSECRET"}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: duplicate key");
        assert!(!err.message().contains("RAWJSONSECRET"));
        let nested = parse_document(
            "f.json",
            br#"{"logGroups":[{"logGroupName":"/a","storedBytes":1,"storedBytes":2}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(nested.message(), "f.json: duplicate key");
        assert!(!nested.message().contains('2'));
        let nested = parse_document(
            "f.json",
            br#"{"logGroups":[{"logGroupName":"/a","storedBytes":1,"retentionInDays":14,"retentionInDays":null}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(nested.message(), "f.json: duplicate key");
        assert!(!nested.message().contains("null"));
    }

    #[test]
    fn ambiguous_region_and_shape_are_rejected() {
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1,"arn":"arn:aws:logs:eu-west-1:999888777666:log-group:/a","logGroupArn":"arn:aws:logs:us-east-1:111122223333:log-group:/a"}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: /a: both arn and logGroupArn");
        assert!(!err.message().contains("999888777666"));
        assert!(!err.message().contains("111122223333"));
        let same = doc(
            r#"{"logGroupName":"/a","storedBytes":1,"arn":"arn:aws:logs:eu-west-1:999888777666:log-group:/a:*","logGroupArn":"arn:aws:logs:eu-west-1:999888777666:log-group:/a"}"#,
        );
        assert_eq!(same.groups[0].region.as_deref(), Some("eu-west-1"));
        let kept = doc(
            r#"{"logGroupName":"/a","storedBytes":1,"logGroupArn":"arn:aws:logs:eu-west-1:999888777666:log-group:/a"}"#,
        );
        assert_eq!(kept.groups[0].region.as_deref(), Some("eu-west-1"));
        assert!(!format!("{:?}", kept.groups[0]).contains("999888777666"));
        let err = parse_document(
            "f.json",
            br#"{"logGroups":[{"logGroupName":"/a","storedBytes":1}],"logGroupName":"/z","storedBytes":9}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: both logGroups and logGroupName");
        assert!(!err.message().contains("/z"));
        assert!(!err.message().contains('9'));
    }

    #[test]
    fn surrogate_pair_is_json() {
        let parsed = doc(
            r#"{"logGroups":[{"logGroupName":"/a","storedBytes":1,"kmsKeyId":"k\uD83D\uDE00"}]}"#,
        );
        assert_eq!(parsed.groups[0].stored_bytes, 1);
        let parsed = doc(r#"{"logGroupName":"/a","storedBytes":1,"x\uD83D\uDE00":1}"#);
        assert_eq!(parsed.groups[0].name, "/a");
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a\uD83D\uDE00","storedBytes":1}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: bad log group name");
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1,"k":"\uD83D"}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: not JSON");
    }

    fn assert_long_string_is_linear(unit: &str, count: usize) {
        let note = unit.repeat(count);
        let json = format!(r#"{{"logGroupName":"/a","storedBytes":1,"note":"{note}"}}"#);
        let start = std::time::Instant::now();
        let parsed = parse_document("many.json", json.as_bytes(), true).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(parsed.groups[0].stored_bytes, 1);
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "scan took {elapsed:?}"
        );
    }

    #[test]
    fn long_string_parses_quickly() {
        assert_long_string_is_linear("a", 2_000_000);
    }

    #[test]
    fn long_non_ascii_string_parses_quickly() {
        // U+20AC is three bytes in UTF-8; a quadratic non-ASCII branch must still fail the bound.
        assert_long_string_is_linear("\u{20AC}", 2_000_000);
    }

    #[test]
    fn bad_class_is_not_echoed_and_arn_keeps_only_the_region() {
        let err = parse_document(
            "f.json",
            br#"{"logGroupName":"/a","storedBytes":1,"logGroupClass":"SECRETCLASS"}"#,
            true,
        )
        .unwrap_err();
        assert!(err.message().contains("bad log group class"));
        assert!(!err.message().contains("SECRETCLASS"));
        let parsed = doc(
            r#"{"logGroupName":"/a","storedBytes":1,"arn":"arn:aws:logs:eu-west-1:999888777666:log-group:/a"}"#,
        );
        assert_eq!(parsed.groups[0].region.as_deref(), Some("eu-west-1"));
        let debug = format!("{:?}", parsed.groups[0]);
        assert!(!debug.contains("999888777666"));
    }

    #[test]
    fn field_errors_name_file_group_and_field() {
        let err =
            parse_document("f.json", br#"{"logGroups":[{"storedBytes":1}]}"#, true).unwrap_err();
        assert_eq!(err.message(), "f.json: missing logGroupName");
        let err = parse_document("f.json", br#"{"logGroupName":"/a"}"#, true).unwrap_err();
        assert_eq!(err.message(), "f.json: /a: missing storedBytes");
        let negative = br#"{"logGroupName":"/a","storedBytes":1,"creationTime":-1}"#;
        let err = parse_document("f.json", negative, true).unwrap_err();
        assert_eq!(err.message(), "f.json: /a: bad creationTime");
        let huge = br#"{"logGroupName":"/a","storedBytes":1,"creationTime":9223372036854775808}"#;
        let err = parse_document("f.json", huge, true).unwrap_err();
        assert_eq!(err.message(), "f.json: /a: bad creationTime");
        let arn = br#"{"logGroupName":"/a","storedBytes":1,"arn":5}"#;
        let err = parse_document("f.json", arn, true).unwrap_err();
        assert_eq!(err.message(), "f.json: /a: bad arn");
    }

    #[test]
    fn empty_strict_and_schema_errors() {
        let err = parse_document("f.json", b"", true).unwrap_err();
        assert!(err.message().contains("no log groups"));
        let err = parse_document("f.json", b"{", true).unwrap_err();
        assert_eq!(err.message(), "f.json: not JSON");
        let err = parse_document("f.json", b"null", true).unwrap_err();
        assert_eq!(err.message(), "f.json: expected log groups");
        let skipped = parse_document("dir/empty.json", b"{\"logGroups\":[]}", false).unwrap();
        assert!(skipped.groups.is_empty());
        assert!(skipped.warnings[0].contains("no log groups"));
    }

    #[test]
    fn unicode_whitespace_only_is_not_empty() {
        for text in ["\u{a0}", "\u{3000}", "\u{2028}", " \u{a0}\n"] {
            let err = parse_document("f.json", text.as_bytes(), true).unwrap_err();
            assert_eq!(err.message(), "f.json: not JSON");
            let skipped = parse_document("f.json", text.as_bytes(), false).unwrap();
            assert!(skipped.groups.is_empty());
            assert_eq!(skipped.warnings.len(), 1);
            assert!(skipped.warnings[0].contains("not JSON"));
        }
        let ascii = parse_document("f.json", b" \t\r\n", false).unwrap();
        assert!(ascii.warnings[0].contains("no log groups"));
    }

    #[test]
    fn name_alphabet_bounds() {
        assert!(valid_log_group_name("/aws/lambda/a_b-c.d#e"));
        assert!(valid_log_group_name(&"a".repeat(512)));
        assert!(!valid_log_group_name(&"a".repeat(513)));
        assert!(!valid_log_group_name(""));
        assert!(!valid_log_group_name("has space"));
    }

    #[test]
    fn large_integer_is_not_rounded() {
        let parsed = doc(r#"{"logGroupName":"/a","storedBytes":9007199254740995}"#);
        assert_eq!(parsed.groups[0].stored_bytes, 9_007_199_254_740_995);
    }

    #[test]
    fn claim_c21_gzip_is_by_magic_bytes() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let body = r#"{"logGroupName":"/a","storedBytes":8}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body.as_bytes()).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let parsed = parse_document("export.txt", &bytes, true).unwrap();
        assert_eq!(parsed.groups[0].stored_bytes, 8);
        let named = parse_document("notes.gz", body.as_bytes(), true).unwrap();
        assert_eq!(named.groups[0].stored_bytes, 8);
        let mut corrupt = vec![0x1f, 0x8b];
        corrupt.extend(b"SECRETLEAK");
        let err = parse_document("notes.txt", &corrupt, true).unwrap_err();
        assert!(err.message().contains("could not read gzip"));
        assert!(!err.message().contains("SECRETLEAK"));
    }

    #[test]
    fn gzip_bytes_after_the_first_member_are_an_error() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let gz = |body: &str| {
            let mut e = GzEncoder::new(Vec::new(), Compression::fast());
            e.write_all(body.as_bytes()).unwrap();
            e.finish().unwrap()
        };
        let m1 = gz(r#"{"logGroups":[{"logGroupName":"/m1","storedBytes":1}]}"#);
        let m2 = gz(r#"{"logGroups":[{"logGroupName":"/m2","storedBytes":2}]}"#);
        assert!(parse_document("m1.gz", &m1, true).is_ok());
        let mut multi = m1.clone();
        multi.extend(&m2);
        let mut garbage = m1.clone();
        garbage.extend(b"GARBAGE");
        let mut truncated = m1.clone();
        truncated.extend(&m2[..m2.len() / 2]);
        for bytes in [multi, garbage, truncated] {
            let err = parse_document("multi.gz", &bytes, true).unwrap_err();
            assert!(err.message().contains("bytes after the first gzip member"));
        }
    }

    #[test]
    fn bom_is_accepted() {
        let text = format!("\u{feff}{}", r#"{"logGroupName":"/a","storedBytes":1}"#);
        let parsed = parse_document("f.json", text.as_bytes(), true).unwrap();
        assert_eq!(parsed.groups.len(), 1);
    }

    #[test]
    fn strict_not_utf8_is_an_error_without_the_bytes() {
        let mut bytes = br#"{"logGroupName":"/a","storedBytes":1}"#.to_vec();
        bytes.extend(b"\xff\xfeSECRETLEAK");
        let err = parse_document("f.json", &bytes, true).unwrap_err();
        assert_eq!(err.message(), "f.json: not UTF-8");
        let lax = parse_document("f.json", &bytes, false).unwrap();
        assert_eq!(lax.warnings, vec!["skipped f.json: not UTF-8".to_string()]);
    }

    #[test]
    fn gzip_that_expands_past_the_limit_is_an_error() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        let chunk = vec![b' '; 1 << 20];
        for _ in 0..=256 {
            encoder.write_all(&chunk).unwrap();
        }
        let bytes = encoder.finish().unwrap();
        assert!(bytes.len() < 64 << 20);
        let err = parse_document("bomb.gz", &bytes, true).unwrap_err();
        assert_eq!(
            err.message(),
            "bomb.gz: file is larger than 268435456 bytes"
        );
    }
}
