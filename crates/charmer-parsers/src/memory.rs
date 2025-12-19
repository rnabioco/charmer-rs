//! Memory parsing utilities for scheduler output.

/// Memory format variants for different schedulers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFormat {
    /// SLURM format: "4G", "1000M", "4096K" (no spaces)
    Slurm,
    /// SLURM sacct format: "4Gn", "1000Mc" (with per-node/per-core suffix)
    SlurmSacct,
    /// LSF format: "4 GB", "1000 MB" (with spaces)
    Lsf,
}

/// Parse memory string to megabytes.
///
/// Handles various formats from SLURM and LSF:
/// - SLURM: "4G", "1000M", "4096K", "4096" (no spaces)
/// - SLURM sacct: "4Gn", "1000Mc" (n=per node, c=per core)
/// - LSF: "4 GB", "1000 MB" (with spaces)
///
/// Returns None for empty strings or placeholder values.
pub fn parse_memory_mb(s: &str, format: MemoryFormat) -> Option<u64> {
    if s.is_empty() || s == "-" {
        return None;
    }

    match format {
        MemoryFormat::Slurm => parse_slurm_memory(s),
        MemoryFormat::SlurmSacct => parse_slurm_sacct_memory(s),
        MemoryFormat::Lsf => parse_lsf_memory(s),
    }
}

/// Parse SLURM squeue memory format (e.g., "4G", "1000M", "4096").
fn parse_slurm_memory(s: &str) -> Option<u64> {
    let s = s.trim();

    if let Some(stripped) = s.strip_suffix('G') {
        stripped.parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(stripped) = s.strip_suffix('M') {
        stripped.parse::<u64>().ok()
    } else if let Some(stripped) = s.strip_suffix('K') {
        stripped.parse::<u64>().ok().map(|v| v / 1024)
    } else {
        // Assume MB if no suffix
        s.parse::<u64>().ok()
    }
}

/// Parse SLURM sacct memory format (e.g., "4Gn", "1000Mc").
fn parse_slurm_sacct_memory(s: &str) -> Option<u64> {
    // sacct memory can have 'n' or 'c' suffix (per node/per core)
    let s = s.trim().trim_end_matches('n').trim_end_matches('c');
    parse_slurm_memory(s)
}

/// Parse LSF memory format (e.g., "4 GB", "1000 MB").
fn parse_lsf_memory(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let value: f64 = parts[0].parse().ok()?;
    let unit = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();

    match unit.as_str() {
        "GB" | "G" => Some((value * 1024.0) as u64),
        "MB" | "M" | "" => Some(value as u64),
        "KB" | "K" => Some((value / 1024.0) as u64),
        _ => Some(value as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_memory() {
        assert_eq!(parse_memory_mb("4G", MemoryFormat::Slurm), Some(4096));
        assert_eq!(parse_memory_mb("1000M", MemoryFormat::Slurm), Some(1000));
        assert_eq!(parse_memory_mb("4096K", MemoryFormat::Slurm), Some(4));
        assert_eq!(parse_memory_mb("4096", MemoryFormat::Slurm), Some(4096));
        assert_eq!(parse_memory_mb("", MemoryFormat::Slurm), None);
    }

    #[test]
    fn test_parse_slurm_sacct_memory() {
        assert_eq!(parse_memory_mb("4Gn", MemoryFormat::SlurmSacct), Some(4096));
        assert_eq!(
            parse_memory_mb("1000Mc", MemoryFormat::SlurmSacct),
            Some(1000)
        );
        assert_eq!(
            parse_memory_mb("4096", MemoryFormat::SlurmSacct),
            Some(4096)
        );
    }

    #[test]
    fn test_parse_lsf_memory() {
        assert_eq!(parse_memory_mb("4 GB", MemoryFormat::Lsf), Some(4096));
        assert_eq!(parse_memory_mb("1000 MB", MemoryFormat::Lsf), Some(1000));
        assert_eq!(parse_memory_mb("1000", MemoryFormat::Lsf), Some(1000));
        assert_eq!(parse_memory_mb("-", MemoryFormat::Lsf), None);
    }
}
