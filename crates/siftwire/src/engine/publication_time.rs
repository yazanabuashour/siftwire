use std::sync::LazyLock;

use jiff::{Timestamp, fmt::rfc2822::DateTimeParser};
use regex::Regex;

pub(super) fn parse_rfc2822(value: &str) -> Option<Timestamp> {
    static STAMP: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
        Regex::new(concat!(
            r"\A\s*((?:[A-Za-z]{3},\s*)?[0-9]{1,2}\s+[A-Za-z]{3}\s+[0-9]{2,})",
            r"\s+([0-9]{2})\s*:\s*([0-9]{2})(?:\s*:([0-5][0-9]))?",
            r"\s+([+-](?:[01][0-9]|2[0-3])[0-5][0-9]|(?i-u:UT|GMT|[ECMP][SD]T|[A-IK-Z]))((?s:.*))\z"
        ))
    });
    let captures = STAMP.as_ref().ok()?.captures(value)?;
    let comments = captures.get(6)?.as_str();
    if !valid_comments(comments) {
        return None;
    }
    let mut date = captures.get(1)?.as_str().split_whitespace();
    let year = date.next_back()?;
    let extra = year.len().saturating_sub(4);
    if !year.get(..extra)?.bytes().all(|digit| digit == b'0') {
        return None;
    }
    let year = year.get(extra..)?;
    let date = date.collect::<Vec<_>>().join(" ");
    let hour = captures.get(2)?.as_str();
    let minute = captures.get(3)?.as_str();
    let zone = captures.get(5)?.as_str();
    let second = captures.get(4).map_or("00", |part| part.as_str());
    DateTimeParser::new()
        .parse_timestamp(format!("{date} {year} {hour}:{minute}:{second} {zone}"))
        .ok()
}

fn valid_comments(mut value: &str) -> bool {
    while !value.is_empty() {
        let Some(comment) = value.trim_start().strip_prefix('(') else {
            return false;
        };
        let mut depth = 1_usize;
        let mut escaped = false;
        let end = comment.bytes().position(|byte| {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'(' => depth = depth.saturating_add(1),
                    b')' => depth = depth.saturating_sub(1),
                    b'\\' => escaped = true,
                    _ => {}
                }
            }
            depth == 0
        });
        let Some(rest) = end.and_then(|index| comment.get(index.saturating_add(1)..)) else {
            return false;
        };
        value = rest;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::parse_rfc2822;

    #[test]
    fn keeps_rss_date_validation_and_legacy_offsets() {
        for value in [
            "Thu, 29 Feb 2024 12:30:00 +0530",
            "Thu, 29 Feb 24 12:30 +0530 (a(nested) comment)(more)",
            "Thu, 29 Feb 02024 12:30:00 +0530",
        ] {
            assert_eq!(
                parse_rfc2822(value)
                    .map(|stamp| stamp.to_string())
                    .as_deref(),
                Some("2024-02-29T07:00:00Z"),
                "{value}"
            );
        }
        for value in [
            "Sat, 31 Dec 2016 23:59:60 GMT",
            "Fri, 29 Feb 2024 12:30:00 +0530",
            "Thu, 29 Feb 2024 12:30:00 XYZ",
            "Thu , 29 Feb 2024 12:30:00 +0530",
            "Thu, 29 Feb 2024 12:30:00 +2400",
            "Thu, 29 Feb 2024 12:30: 00 +0530",
            "Thu, 29 Feb 2024 12:30:00 +0530 (unclosed",
        ] {
            assert!(parse_rfc2822(value).is_none(), "{value}");
        }
    }
}
