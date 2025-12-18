//! Parser for main snakemake log file (.snakemake/log/*.snakemake.log).
//!
//! Extracts pipeline-level information like total job count and progress.

use camino::Utf8Path;
use std::collections::HashMap;
use std::fs;
use std::io;

/// Information parsed from the main snakemake log.
#[derive(Debug, Clone, Default)]
pub struct SnakemakeLogInfo {
    /// Total number of jobs in the pipeline
    pub total_jobs: Option<usize>,
    /// Number of completed jobs
    pub completed_jobs: usize,
    /// Job counts per rule
    pub jobs_by_rule: HashMap<String, usize>,
    /// Number of cores being used
    pub cores: Option<usize>,
    /// Host machine name
    pub host: Option<String>,
    /// Whether the pipeline has finished
    pub finished: bool,
    /// Whether there were errors
    pub has_errors: bool,
    /// Error messages found
    pub errors: Vec<String>,
}

impl SnakemakeLogInfo {
    /// Get progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        match self.total_jobs {
            Some(total) if total > 0 => self.completed_jobs as f64 / total as f64,
            _ => 0.0,
        }
    }

    /// Get progress as a percentage string.
    pub fn progress_percent(&self) -> String {
        format!("{:.0}%", self.progress() * 100.0)
    }
}

/// Find the most recent snakemake log file in the working directory.
pub fn find_latest_log(working_dir: &Utf8Path) -> Option<camino::Utf8PathBuf> {
    let log_dir = working_dir.join(".snakemake").join("log");
    if !log_dir.exists() {
        return None;
    }

    let mut latest: Option<(std::time::SystemTime, camino::Utf8PathBuf)> = None;

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".snakemake.log") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(utf8_path) = camino::Utf8PathBuf::try_from(path) {
                                if latest.is_none() || modified > latest.as_ref().unwrap().0 {
                                    latest = Some((modified, utf8_path));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    latest.map(|(_, path)| path)
}

/// Parse the main snakemake log file.
pub fn parse_main_log(working_dir: &Utf8Path) -> io::Result<SnakemakeLogInfo> {
    let log_path = find_latest_log(working_dir)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No snakemake log file found"))?;

    parse_log_file(&log_path)
}

/// Parse a specific snakemake log file.
pub fn parse_log_file(path: &Utf8Path) -> io::Result<SnakemakeLogInfo> {
    let content = fs::read_to_string(path)?;
    Ok(parse_log_content(&content))
}

/// Parse snakemake log content.
pub fn parse_log_content(content: &str) -> SnakemakeLogInfo {
    let mut info = SnakemakeLogInfo::default();
    let mut in_job_stats = false;

    for line in content.lines() {
        let line = line.trim();

        // Parse host
        if line.starts_with("host:") {
            info.host = Some(line.trim_start_matches("host:").trim().to_string());
            continue;
        }

        // Parse cores
        if line.starts_with("Provided cores:") {
            if let Some(cores_str) = line.strip_prefix("Provided cores:") {
                info.cores = cores_str.trim().parse().ok();
            }
            continue;
        }

        // Detect job stats section
        if line == "Job stats:" {
            in_job_stats = true;
            continue;
        }

        // Parse job stats table
        if in_job_stats {
            // End of table (empty line or next section)
            if line.is_empty() || line.starts_with("Select jobs") {
                in_job_stats = false;
                continue;
            }

            // Skip header line
            if line.starts_with("job") || line.starts_with("---") {
                continue;
            }

            // Parse "rule_name    count" lines
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let rule = parts[0];
                if let Ok(count) = parts[parts.len() - 1].parse::<usize>() {
                    if rule == "total" {
                        info.total_jobs = Some(count);
                    } else {
                        info.jobs_by_rule.insert(rule.to_string(), count);
                    }
                }
            }
            continue;
        }

        // Parse progress: "X of Y steps (Z%) done"
        if line.contains(" of ") && line.contains(" steps") && line.contains("done") {
            // Extract "X of Y"
            if let Some(of_idx) = line.find(" of ") {
                let before = &line[..of_idx];
                // Find the last number before " of "
                let completed: Option<usize> = before
                    .split_whitespace()
                    .last()
                    .and_then(|s| s.parse().ok());

                if let Some(c) = completed {
                    info.completed_jobs = c;
                }
            }
            continue;
        }

        // Detect completion
        if line.contains("steps (100%) done") || line.contains("Nothing to be done") {
            info.finished = true;
            continue;
        }

        // Detect errors
        if line.starts_with("Error") || line.contains("error:") || line.contains("Exception") {
            info.has_errors = true;
            if line.len() < 200 {
                info.errors.push(line.to_string());
            }
        }

        // Specific error patterns
        if line.contains("Exiting because a job execution failed") {
            info.has_errors = true;
            info.errors.push(line.to_string());
        }
    }

    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_stats() {
        let content = r#"
Building DAG of jobs...
Job stats:
job                      count
---------------------  -------
align_sample                 4
process_sample               4
total                        8

Select jobs to execute...
"#;
        let info = parse_log_content(content);
        assert_eq!(info.total_jobs, Some(8));
        assert_eq!(info.jobs_by_rule.get("align_sample"), Some(&4));
        assert_eq!(info.jobs_by_rule.get("process_sample"), Some(&4));
    }

    #[test]
    fn test_parse_progress() {
        let content = r#"
[Thu Dec 18 12:24:21 2025]
Finished jobid: 22 (Rule: call_variants)
5 of 27 steps (19%) done
"#;
        let info = parse_log_content(content);
        assert_eq!(info.completed_jobs, 5);
    }

    #[test]
    fn test_parse_cores() {
        let content = "Provided cores: 4\n";
        let info = parse_log_content(content);
        assert_eq!(info.cores, Some(4));
    }

    #[test]
    fn test_parse_host() {
        let content = "host: myserver\n";
        let info = parse_log_content(content);
        assert_eq!(info.host, Some("myserver".to_string()));
    }
}
