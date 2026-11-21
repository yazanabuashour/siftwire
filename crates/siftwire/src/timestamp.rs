use std::sync::LazyLock;

use anyhow::{Context, Result};
use jiff::Timestamp;
use regex::Regex;

// Guard the process contract against additional ISO spellings and leap-second clamping.
pub fn parse(value: &str) -> Result<Timestamp> {
    static STAMP: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
        Regex::new(concat!(
            r"\A([0-9]{4}-[0-9]{2}-[0-9]{2}[Tt ][0-9]{2}:[0-9]{2}:[0-5][0-9])",
            r"(?:(\.[0-9]{1,9})[0-9]*)?([Zz]|[+\-−](?:[01][0-9]|2[0-3]):[0-5][0-9])\z"
        ))
    });
    let stamp = STAMP
        .as_ref()
        .map_err(Clone::clone)
        .context("compile timestamp pattern")?;
    let captures = stamp.captures(value).context("invalid RFC 3339 syntax")?;
    let head = captures
        .get(1)
        .context("missing RFC 3339 date and time")?
        .as_str();
    let fraction = captures.get(2);
    let offset = captures.get(3).context("missing RFC 3339 offset")?.as_str();
    let suffix = offset.strip_prefix('−');
    let digits = fraction.map_or("", |part| part.as_str());
    if suffix.is_none()
        && head
            .len()
            .saturating_add(digits.len())
            .saturating_add(offset.len())
            == value.len()
    {
        return Ok(value.parse()?);
    }
    let normalized = suffix.map_or_else(
        || format!("{head}{digits}{offset}"),
        |suffix| format!("{head}{digits}-{suffix}"),
    );
    Ok(normalized.parse()?)
}

pub fn wire(value: Timestamp) -> String {
    let precision = match value.subsec_nanosecond() {
        0 => 0,
        nanos if nanos % 1_000_000 == 0 => 3,
        nanos if nanos % 1_000 == 0 => 6,
        _ => 9,
    };
    format!("{value:.precision$}")
}

#[cfg(test)]
mod tests {
    use super::{parse, wire};

    #[test]
    fn preserves_wire_precision_and_legacy_iso_spellings() {
        for (input, expected) in [
            ("2024-02-29t12:30:00z", "2024-02-29T12:30:00Z"),
            ("2024-02-29 12:30:00.1+05:30", "2024-02-29T07:00:00.100Z"),
            (
                "2024-02-29T23:30:00.1234−02:00",
                "2024-03-01T01:30:00.123400Z",
            ),
            (
                "2024-02-29T12:30:00.123456789999-00:00",
                "2024-02-29T12:30:00.123456789Z",
            ),
            ("1969-12-31T23:59:59.1Z", "1969-12-31T23:59:59.100Z"),
        ] {
            assert_eq!(
                parse(input)
                    .as_ref()
                    .map(|value| wire(*value))
                    .ok()
                    .as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn rejects_leap_seconds_and_non_contract_iso_spellings() {
        for value in [
            "2016-12-31T23:59:60Z",
            "2023-02-29T12:30:00Z",
            "2024-02-29T12:30Z",
            "2024-02-29T12:30:000Z",
            "2024-02-29T12:30:00+0530",
            "2024-02-29T12:30:00+24:00",
            "2024-02-29T12:30:00,1Z",
            "2024-02-29T12:30:00Z[UTC]",
        ] {
            assert!(parse(value).is_err(), "{value}");
        }
    }
}
