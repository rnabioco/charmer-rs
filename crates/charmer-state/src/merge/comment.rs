//! Comment/description field parsing for snakemake job correlation.

/// Parse snakemake SLURM comment field: "rule_{rulename}_wildcards_{wildcards}"
pub fn parse_slurm_comment(comment: &str) -> Option<(String, Option<String>)> {
    // Format: "rule_RULENAME_wildcards_WILDCARDS" or just "rule_RULENAME"
    if !comment.starts_with("rule_") {
        return None;
    }

    let rest = &comment[5..]; // Skip "rule_"

    if let Some(wc_pos) = rest.find("_wildcards_") {
        let rule = &rest[..wc_pos];
        let wildcards = &rest[wc_pos + 11..]; // Skip "_wildcards_"
        Some((
            rule.to_string(),
            if wildcards.is_empty() {
                None
            } else {
                Some(wildcards.to_string())
            },
        ))
    } else {
        Some((rest.to_string(), None))
    }
}

/// Parse snakemake LSF job description field (same format as SLURM comment).
pub fn parse_lsf_description(desc: &str) -> Option<(String, Option<String>)> {
    // LSF snakemake executor uses same format as SLURM
    parse_slurm_comment(desc)
}

/// Generate a job ID from rule and wildcards.
pub fn make_job_id(rule: &str, wildcards: Option<&str>) -> String {
    match wildcards {
        Some(wc) => format!("{}[{}]", rule, wc),
        None => rule.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_comment() {
        // Basic rule only
        let (rule, wc) = parse_slurm_comment("rule_align_reads").unwrap();
        assert_eq!(rule, "align_reads");
        assert!(wc.is_none());

        // Rule with wildcards
        let (rule, wc) = parse_slurm_comment("rule_align_reads_wildcards_sample=S1").unwrap();
        assert_eq!(rule, "align_reads");
        assert_eq!(wc.unwrap(), "sample=S1");

        // Invalid format
        assert!(parse_slurm_comment("not_a_rule").is_none());
    }

    #[test]
    fn test_make_job_id() {
        assert_eq!(make_job_id("align", None), "align");
        assert_eq!(make_job_id("align", Some("sample=S1")), "align[sample=S1]");
    }
}
