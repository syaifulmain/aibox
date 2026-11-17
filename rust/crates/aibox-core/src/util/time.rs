//! ISO-8601 date/time formatting matching `Qt::ISODate`.

use chrono::{DateTime, Local, Utc};

/// Formats a local timestamp as `YYYY-MM-DDTHH:MM:SS` (no offset or subseconds),
/// matching `QDateTime::currentDateTime().toString(Qt::ISODate)`.
pub fn format_local_iso(dt: DateTime<Local>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Formats a UTC timestamp as `YYYY-MM-DDTHH:MM:SSZ`,
/// matching `QDateTime::currentDateTimeUtc().toString(Qt::ISODate)`.
pub fn format_utc_iso(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Returns the current local time formatted with `Qt::ISODate`.
pub fn now_local_iso() -> String {
    format_local_iso(Local::now())
}

/// Returns the current UTC time formatted with `Qt::ISODate`.
pub fn now_utc_iso() -> String {
    format_utc_iso(Utc::now())
}

/// Returns the current local time formatted as `YYYYMMDD-HHMMSS`.
pub fn now_compact_stamp() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Returns the current Unix timestamp in seconds.
pub fn now_epoch_secs() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn formats_utc_iso_without_subseconds() {
        let dt = Utc.with_ymd_and_hms(2026, 11, 17, 18, 2, 56).unwrap();
        assert_eq!(format_utc_iso(dt), "2026-11-17T18:02:56Z");
    }
}
