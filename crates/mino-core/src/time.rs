//! Timestamps, without a calendar dependency.
//!
//! The journal needs two things: an ordered id, and a string a person can read.
//! `chrono` would give us both, but it reaches for the Windows API to work out
//! the local time zone — and this crate's whole claim is that it touches no OS.
//! Everything here is UTC, derived from `SystemTime`, so the claim holds.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Utc {
    pub seconds: i64,
    pub millis: u32,
}

impl Utc {
    pub fn now() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => Utc {
                seconds: d.as_secs() as i64,
                millis: d.subsec_millis(),
            },
            // A clock set before 1970 is not worth failing an apply over.
            Err(_) => Utc {
                seconds: 0,
                millis: 0,
            },
        }
    }

    pub fn from_seconds(seconds: i64) -> Self {
        Utc { seconds, millis: 0 }
    }

    fn parts(&self) -> (i64, u32, u32, u32, u32, u32) {
        let days = self.seconds.div_euclid(86_400);
        let secs_of_day = self.seconds.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        (
            y,
            m,
            d,
            (secs_of_day / 3600) as u32,
            ((secs_of_day % 3600) / 60) as u32,
            (secs_of_day % 60) as u32,
        )
    }

    /// `2026-08-28T14:31:07.482Z` — sorts lexicographically in time order, and
    /// `new Date(...)` in the UI parses it without help.
    pub fn rfc3339(&self) -> String {
        let (y, mo, d, h, mi, s) = self.parts();
        format!(
            "{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{:03}Z",
            self.millis
        )
    }

    /// `20260828T143107482` — the journal entry id, and therefore a file name.
    pub fn compact(&self) -> String {
        let (y, mo, d, h, mi, s) = self.parts();
        format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}{:03}", self.millis)
    }
}

/// Days since the Unix epoch to a civil date, by Howard Hinnant's algorithm.
/// Correct for the whole proleptic Gregorian calendar, which is considerably
/// more than we need, but it is the version that is known to be right.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> String {
        Utc::from_seconds(seconds).rfc3339()
    }

    #[test]
    fn the_epoch() {
        assert_eq!(at(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn known_instants() {
        // Checked against `date -u -d @<seconds>`.
        assert_eq!(at(1_000_000_000), "2001-09-09T01:46:40.000Z");
        assert_eq!(at(1_772_000_000), "2026-02-25T06:13:20.000Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00.000Z"); // leap day
        assert_eq!(at(1_709_164_800), "2024-02-29T00:00:00.000Z");
        assert_eq!(at(1_767_225_599), "2025-12-31T23:59:59.000Z");
        assert_eq!(at(1_767_225_600), "2026-01-01T00:00:00.000Z");
    }

    #[test]
    fn millis_are_kept() {
        let t = Utc {
            seconds: 1_767_225_600,
            millis: 7,
        };
        assert_eq!(t.rfc3339(), "2026-01-01T00:00:00.007Z");
        assert_eq!(t.compact(), "20260101T000000007");
    }

    #[test]
    fn strings_sort_in_time_order() {
        let mut stamps = [at(1_767_225_600), at(0), at(1_000_000_000)];
        stamps.sort();
        assert_eq!(stamps[0], at(0));
        assert_eq!(stamps[2], at(1_767_225_600));
    }

    #[test]
    fn now_is_in_this_century() {
        let now = Utc::now().rfc3339();
        assert!(now.starts_with("20"), "unexpected clock reading: {now}");
    }
}
