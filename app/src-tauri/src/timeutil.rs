//! Small time helpers (replaces `chrono`, RUSTSEC-2026-0543 unmaintained).
//! Backed by the `time` crate, which is already in the dependency tree via
//! Tauri.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Current Unix time in seconds (UTC).
pub fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

/// Parse an RFC 3339 timestamp to Unix seconds. `None` when malformed.
pub fn parse_rfc3339_unix(s: &str) -> Option<i64> {
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(|t| t.unix_timestamp())
}

/// Format as chrono's `Utc::to_rfc3339()` did: `YYYY-MM-DDTHH:MM:SS`,
/// then 3/6/9 fractional digits only when non-zero, then `+00:00`.
pub fn rfc3339_utc(unix_nanos: i128) -> String {
    let t =
        OffsetDateTime::from_unix_timestamp_nanos(unix_nanos).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let ns = t.nanosecond();
    let frac = if ns == 0 {
        String::new()
    } else if ns.is_multiple_of(1_000_000) {
        format!(".{:03}", ns / 1_000_000)
    } else if ns.is_multiple_of(1_000) {
        format!(".{:06}", ns / 1_000)
    } else {
        format!(".{ns:09}")
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{frac}+00:00",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// Current UTC time in chrono's `to_rfc3339()` format.
pub fn now_rfc3339() -> String {
    rfc3339_utc(OffsetDateTime::now_utc().unix_timestamp_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_pins_chrono_layout() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(
            rfc3339_utc(1_778_184_000_000_000_000),
            "2026-05-07T20:00:00+00:00"
        );
        assert_eq!(
            rfc3339_utc(1_778_184_000_123_000_000),
            "2026-05-07T20:00:00.123+00:00"
        );
        assert_eq!(
            rfc3339_utc(1_778_184_000_123_456_000),
            "2026-05-07T20:00:00.123456+00:00"
        );
        assert_eq!(
            rfc3339_utc(1_778_184_000_123_456_789),
            "2026-05-07T20:00:00.123456789+00:00"
        );
    }

    #[test]
    fn parse_handles_offsets_fractions_and_garbage() {
        assert_eq!(parse_rfc3339_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_unix("2026-05-07T20:00:00+00:00"),
            Some(1_778_184_000)
        );
        assert_eq!(
            parse_rfc3339_unix("2026-05-07T22:00:00+02:00"),
            Some(1_778_184_000)
        );
        assert_eq!(
            parse_rfc3339_unix("2026-05-07T20:00:00.987Z"),
            Some(1_778_184_000)
        );
        assert_eq!(parse_rfc3339_unix("2026-05-07 20:00:00"), None);
        assert_eq!(parse_rfc3339_unix(""), None);
    }

    #[test]
    fn now_is_sane() {
        assert!(now_unix() > 1_700_000_000);
        assert!(now_rfc3339().ends_with("+00:00"));
    }
}
