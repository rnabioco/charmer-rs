//! Time parsing utilities for scheduler output.

use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;

/// Parse a SLURM timestamp (YYYY-MM-DDTHH:MM:SS or placeholder values).
///
/// Returns None for empty strings or placeholder values like "N/A", "Unknown", "None".
pub fn parse_slurm_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == "N/A" || s == "Unknown" || s == "None" {
        return None;
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .and_then(|dt| Utc.from_local_datetime(&dt).single())
}

/// Parse an LSF timestamp format (Mon DD HH:MM or Mon DD HH:MM YYYY).
///
/// Returns None for empty strings or "-" placeholder.
pub fn parse_lsf_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == "-" {
        return None;
    }

    let current_year = Utc::now().year();

    // Try with year first (e.g., "Dec 18 10:30 2024")
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%b %d %H:%M %Y") {
        return Utc.from_local_datetime(&dt).single();
    }

    // Try without year, assume current year (e.g., "Dec 18 10:30")
    if let Ok(dt) =
        NaiveDateTime::parse_from_str(&format!("{} {}", s, current_year), "%b %d %H:%M %Y")
    {
        return Utc.from_local_datetime(&dt).single();
    }

    None
}

/// Parse a duration in various formats.
///
/// Supports:
/// - D-HH:MM:SS (SLURM time limit with days)
/// - HH:MM:SS
/// - MM:SS
/// - Seconds as integer
///
/// Returns None for "UNLIMITED" or empty strings.
pub fn parse_duration(s: &str) -> Option<Duration> {
    if s.is_empty() || s == "UNLIMITED" || s == "-" {
        return None;
    }

    // Check for day separator (D-HH:MM:SS)
    let parts: Vec<&str> = s.split('-').collect();
    let (days, time_part) = if parts.len() == 2 {
        (parts[0].parse::<u64>().unwrap_or(0), parts[1])
    } else {
        (0, parts[0])
    };

    let time_parts: Vec<u64> = time_part
        .split(':')
        .filter_map(|p| p.parse().ok())
        .collect();

    let seconds = match time_parts.len() {
        3 => time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2],
        2 => time_parts[0] * 60 + time_parts[1],
        1 => time_parts[0],
        _ => return None,
    };

    Some(Duration::from_secs(days * 86400 + seconds))
}

/// Parse exit code from SLURM format (exit_code:signal).
///
/// Returns the exit code portion, defaulting to 0 if parsing fails.
pub fn parse_exit_code(s: &str) -> i32 {
    s.split(':')
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_timestamp() {
        let dt = parse_slurm_timestamp("2024-01-15T10:30:00").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");

        assert!(parse_slurm_timestamp("N/A").is_none());
        assert!(parse_slurm_timestamp("Unknown").is_none());
        assert!(parse_slurm_timestamp("None").is_none());
        assert!(parse_slurm_timestamp("").is_none());
    }

    #[test]
    fn test_parse_lsf_timestamp() {
        // With year
        let dt = parse_lsf_timestamp("Dec 18 10:30 2024").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-12-18");

        // Without year (uses current year)
        let dt = parse_lsf_timestamp("Dec 18 10:30");
        assert!(dt.is_some());

        assert!(parse_lsf_timestamp("-").is_none());
        assert!(parse_lsf_timestamp("").is_none());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1:00:00"), Some(Duration::from_secs(3600)));
        assert_eq!(
            parse_duration("1-00:00:00"),
            Some(Duration::from_secs(86400))
        );
        assert_eq!(parse_duration("30:00"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_duration("3600"), Some(Duration::from_secs(3600)));
        assert!(parse_duration("UNLIMITED").is_none());
        assert!(parse_duration("-").is_none());
    }

    #[test]
    fn test_parse_exit_code() {
        assert_eq!(parse_exit_code("0:0"), 0);
        assert_eq!(parse_exit_code("1:0"), 1);
        assert_eq!(parse_exit_code("137:9"), 137);
        assert_eq!(parse_exit_code(""), 0);
    }
}
