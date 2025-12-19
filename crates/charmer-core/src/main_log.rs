//! Parser for main snakemake log file (.snakemake/log/*.snakemake.log).
//!
//! Extracts pipeline-level information like total job count and progress.

use camino::Utf8Path;
use std::collections::{HashMap, HashSet};
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
    /// Rules that have no output files (target rules like "all")
    /// These rules don't create metadata files and need synthetic job entries
    pub target_rules: HashSet<String>,
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

    // State for tracking rule blocks to identify target rules (rules without outputs)
    let mut current_rule: Option<String> = None;
    let mut current_rule_has_output = false;
    // Track all rules we've seen with their output status
    let mut rules_with_outputs: HashSet<String> = HashSet::new();
    let mut all_seen_rules: HashSet<String> = HashSet::new();

    for line in content.lines() {
        let line = line.trim();

        // Detect rule block start: "localrule X:" or "rule X:"
        // This helps us identify target rules (rules without output files)
        if (line.starts_with("localrule ") || line.starts_with("rule "))
            && line.ends_with(':')
            && !line.contains("(Rule:")
        {
            // Save previous rule's output status
            if let Some(ref rule) = current_rule {
                all_seen_rules.insert(rule.clone());
                if current_rule_has_output {
                    rules_with_outputs.insert(rule.clone());
                }
            }

            // Extract rule name: "localrule X:" or "rule X:" -> "X"
            let rule_part = line
                .trim_start_matches("localrule ")
                .trim_start_matches("rule ");
            let rule_name = rule_part.trim_end_matches(':').to_string();
            current_rule = Some(rule_name);
            current_rule_has_output = false;
            continue;
        }

        // Track if current rule has an output line
        if current_rule.is_some() && line.starts_with("output:") {
            current_rule_has_output = true;
        }

        // End of rule block detection: timestamp line or certain keywords
        if current_rule.is_some()
            && (line.starts_with('[') || line.starts_with("Select jobs") || line.is_empty())
        {
            if let Some(ref rule) = current_rule {
                all_seen_rules.insert(rule.clone());
                if current_rule_has_output {
                    rules_with_outputs.insert(rule.clone());
                }
            }
            current_rule = None;
            current_rule_has_output = false;
        }

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

    // Handle any remaining rule block at end of file
    if let Some(ref rule) = current_rule {
        all_seen_rules.insert(rule.clone());
        if current_rule_has_output {
            rules_with_outputs.insert(rule.clone());
        }
    }

    // Target rules are those we've seen in rule blocks that have no outputs
    // These are rules like "all" that just aggregate other targets
    info.target_rules = all_seen_rules
        .difference(&rules_with_outputs)
        .cloned()
        .collect();

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

    #[test]
    fn test_parse_target_rules() {
        // "all" rule has no output - it's a target rule
        // "final_merge" rule has output - it's NOT a target rule
        let content = r#"
[Thu Dec 18 16:20:31 2025]
localrule final_merge:
    input: results/merged/sample1_merged.vcf
    output: results/all_variants.vcf
    log: logs/final_merge.log
    jobid: 2
    reason: Missing output files: results/all_variants.vcf

[Thu Dec 18 16:20:47 2025]
localrule all:
    input: results/final_report.txt
    jobid: 0
    reason: Input files updated by another job: results/final_report.txt
    resources: tmpdir=/tmp
"#;
        let info = parse_log_content(content);
        // "all" should be a target rule (no output)
        assert!(info.target_rules.contains("all"));
        // "final_merge" should NOT be a target rule (has output)
        assert!(!info.target_rules.contains("final_merge"));
    }

    #[test]
    fn test_parse_target_rules_with_regular_rule() {
        let content = r#"
[Thu Dec 18 16:17:43 2025]
rule call_variants:
    input: results/aligned/sample6.bam
    output: results/variants/sample6_chr2.vcf
    log: logs/call_variants/sample6_chr2.log
    jobid: 35
    wildcards: sample=sample6, chrom=chr2

[Thu Dec 18 16:20:47 2025]
localrule all:
    input: results/final_report.txt
    jobid: 0
"#;
        let info = parse_log_content(content);
        assert!(info.target_rules.contains("all"));
        assert!(!info.target_rules.contains("call_variants"));
    }
}
