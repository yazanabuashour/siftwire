use std::sync::LazyLock;

use anyhow::{Result, ensure};
use jiff::tz::{TimeZone, TimeZoneDatabase};

pub const CENTRAL: &str = "America/Chicago";

pub fn named(name: &str) -> Result<TimeZone> {
    static DB: LazyLock<TimeZoneDatabase> = LazyLock::new(TimeZoneDatabase::bundled);
    let zone = DB.get(name)?;
    ensure!(
        zone.iana_name() == Some(name) && name != "Etc/Unknown",
        "invalid IANA time zone {name:?}"
    );
    Ok(zone)
}

pub fn central() -> TimeZone {
    jiff::tz::get!("America/Chicago")
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_instants_with_bundled_iana_rules() {
        for (zone, stamp, expected) in [
            ("America/Chicago", "2026-03-08T07:59:59Z", "01:59:59 CST"),
            ("America/Chicago", "2026-03-08T08:00:00Z", "03:00:00 CDT"),
            ("America/Chicago", "2026-11-01T06:30:00Z", "01:30:00 CDT"),
            ("America/Chicago", "2026-11-01T07:30:00Z", "01:30:00 CST"),
            ("America/Vancouver", "2026-11-21T19:30:00Z", "12:30:00 MST"),
            ("UTC", "2024-02-29T12:30:00Z", "12:30:00 UTC"),
        ] {
            let local = crate::timestamp::parse(stamp)
                .and_then(|value| super::named(zone).map(|zone| value.to_zoned_datetime(zone)));
            assert_eq!(
                local
                    .as_ref()
                    .map(|value| value.strftime("%H:%M:%S %Z").to_string())
                    .ok()
                    .as_deref(),
                Some(expected),
                "{zone} {stamp}"
            );
        }
        assert!(
            super::named("america/chicago").is_err(),
            "legacy zone names are case-sensitive"
        );
    }
}
