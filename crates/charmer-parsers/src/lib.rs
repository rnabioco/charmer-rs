//! Shared parsing utilities for scheduler command output.
//!
//! This crate provides common parsing functions used by both
//! charmer-slurm and charmer-lsf to reduce code duplication.

pub mod command;
pub mod memory;
pub mod time;

pub use command::{run_command, run_command_allow_failure, CommandError};
pub use memory::{parse_memory_mb, MemoryFormat};
pub use time::{parse_duration, parse_exit_code, parse_lsf_timestamp, parse_slurm_timestamp};

/// Filter helper for optional string fields.
/// Returns None if the string is empty or a placeholder value.
pub fn non_empty_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed == "N/A" || trimmed == "Unknown" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Split a pipe-delimited line and validate field count.
pub fn split_delimited(line: &str, min_fields: usize) -> Result<Vec<&str>, String> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < min_fields {
        return Err(format!(
            "Expected {} fields, got {}: {}",
            min_fields,
            fields.len(),
            line
        ));
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_empty_string() {
        assert_eq!(non_empty_string("hello"), Some("hello".to_string()));
        assert_eq!(non_empty_string("  hello  "), Some("hello".to_string()));
        assert_eq!(non_empty_string(""), None);
        assert_eq!(non_empty_string("-"), None);
        assert_eq!(non_empty_string("N/A"), None);
        assert_eq!(non_empty_string("Unknown"), None);
    }

    #[test]
    fn test_split_delimited() {
        let line = "a|b|c|d";
        assert_eq!(split_delimited(line, 4).unwrap(), vec!["a", "b", "c", "d"]);
        assert!(split_delimited(line, 5).is_err());
    }
}
