//! Unified job and pipeline state types.

use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// Pre-compiled regex patterns for environment detection
static PIXI_ENV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"pixi\s+run\s+(?:-e|--environment)\s+(\S+)").unwrap());
static CONDA_ENV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:conda|mamba|micromamba)\s+(?:run\s+(?:-n|--name)|activate)\s+(\S+)").unwrap()
});
static CONTAINER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:singularity|apptainer)\s+exec\s+(\S+)|docker\s+run\s+(?:[^/]+\s+)*(\S+/\S+)")
        .unwrap()
});

// Pre-compiled regex patterns for error extraction
static ERROR_RULE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:error in rule|rule[:\s]+|for rule)\s+(\w+)").unwrap());
static EXIT_CODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:exit\s*code|exitcode|return\s*code)[:\s]+(\d+)|returned\s+(\d+)").unwrap()
});

/// Special job ID for the main snakemake pipeline log.
pub const MAIN_PIPELINE_JOB_ID: &str = "__snakemake_main__";

/// Unified job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Waiting for dependencies
    Pending,
    /// Queued in SLURM
    Queued,
    /// Currently running
    Running,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
    /// Unknown state
    Unknown,
}

/// Trait for converting scheduler-specific states to unified JobStatus.
pub trait ToJobStatus {
    /// Convert to unified JobStatus.
    fn to_job_status(&self) -> JobStatus;

    /// Extract error information if the job failed.
    fn to_job_error(&self) -> Option<JobError>;
}

// Implementation for SLURM job states
impl ToJobStatus for charmer_slurm::SlurmJobState {
    fn to_job_status(&self) -> JobStatus {
        match self {
            Self::Pending => JobStatus::Queued,
            Self::Running => JobStatus::Running,
            Self::Completed { .. } => JobStatus::Completed,
            Self::Failed { .. } => JobStatus::Failed,
            Self::Cancelled => JobStatus::Cancelled,
            Self::Timeout => JobStatus::Failed,
            Self::OutOfMemory => JobStatus::Failed,
            Self::Unknown(_) => JobStatus::Unknown,
        }
    }

    fn to_job_error(&self) -> Option<JobError> {
        match self {
            Self::Failed { exit_code, error } => Some(JobError {
                exit_code: *exit_code,
                message: error.clone(),
                analysis: None, // Will be populated by failure analysis
            }),
            Self::Timeout => Some(JobError {
                exit_code: -1,
                message: "Job exceeded time limit".to_string(),
                analysis: None,
            }),
            Self::OutOfMemory => Some(JobError {
                exit_code: -1,
                message: "Job exceeded memory limit".to_string(),
                analysis: None,
            }),
            _ => None,
        }
    }
}

// Implementation for LSF job states
impl ToJobStatus for charmer_lsf::LsfJobState {
    fn to_job_status(&self) -> JobStatus {
        match self {
            Self::Pending => JobStatus::Queued,
            Self::Running => JobStatus::Running,
            Self::Done { .. } => JobStatus::Completed,
            Self::Exit { .. } => JobStatus::Failed,
            Self::UserSuspendedPending | Self::UserSuspended => JobStatus::Pending,
            Self::SystemSuspended => JobStatus::Pending,
            Self::Zombie => JobStatus::Unknown,
            Self::Unknown(_) => JobStatus::Unknown,
        }
    }

    fn to_job_error(&self) -> Option<JobError> {
        match self {
            Self::Exit { exit_code, error } => Some(JobError {
                exit_code: *exit_code,
                message: error.clone(),
                analysis: None, // Will be populated by failure analysis
            }),
            _ => None,
        }
    }
}

impl JobStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Queued => "◐",
            Self::Running => "●",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::Cancelled => "⊘",
            Self::Unknown => "?",
        }
    }
}

/// Job timing information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobTiming {
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Job resource allocation (requested).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobResources {
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub time_limit: Option<Duration>,
    pub partition: Option<String>,
    pub node: Option<String>,
}

/// Actual resource usage (from sacct/bhist for finished jobs).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Maximum resident set size (actual memory used) in MB
    pub max_rss_mb: Option<u64>,
    /// Actual elapsed runtime in seconds
    pub elapsed_seconds: Option<u64>,
    /// Total CPU time in seconds
    pub cpu_time_seconds: Option<u64>,
}

/// Job error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobError {
    pub exit_code: i32,
    pub message: String,
    /// Detailed failure analysis (if available)
    pub analysis: Option<FailureAnalysis>,
}

/// Detailed failure analysis from SLURM/LSF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    /// Classified failure mode
    pub mode: FailureMode,
    /// Human-readable explanation
    pub explanation: String,
    /// Suggested fix
    pub suggestion: String,
    /// Memory used (MB) if available
    pub memory_used_mb: Option<u64>,
    /// Memory limit (MB) if available
    pub memory_limit_mb: Option<u64>,
    /// Runtime (seconds) if available
    pub runtime_seconds: Option<u64>,
    /// Time limit (seconds) if available
    pub time_limit_seconds: Option<u64>,
}

/// Failure mode classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureMode {
    /// Job ran out of memory
    OutOfMemory,
    /// Job exceeded time limit
    Timeout,
    /// Job failed with exit code
    ExitCode,
    /// Job was cancelled/killed
    Cancelled,
    /// Node/host failure
    NodeFailure,
    /// Unknown failure
    Unknown,
}

/// Execution environment type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvType {
    /// Pixi environment (from `pixi run -e <env>`)
    Pixi,
    /// Conda environment (from `conda run -n <env>` or conda_env metadata)
    Conda,
    /// Container (Singularity/Apptainer/Docker)
    Container,
    /// Direct shell execution (no environment wrapper)
    Direct,
}

/// Execution environment information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnvironment {
    /// Type of environment
    pub env_type: EnvType,
    /// Environment name (e.g., "myenv" for pixi/conda)
    pub env_name: Option<String>,
    /// Container image URL (for containers)
    pub image_url: Option<String>,
}

impl ExecutionEnvironment {
    /// Detect execution environment from job metadata.
    pub fn detect(shellcmd: &str, conda_env: Option<&str>, container_url: Option<&str>) -> Self {
        // Priority: Container > Pixi > Conda > Direct

        // Check for container
        if let Some(url) = container_url {
            return Self {
                env_type: EnvType::Container,
                env_name: None,
                image_url: Some(url.to_string()),
            };
        }

        // Check shellcmd for container patterns
        if let Some(image) = Self::detect_container(shellcmd) {
            return Self {
                env_type: EnvType::Container,
                env_name: None,
                image_url: Some(image),
            };
        }

        // Check shellcmd for pixi pattern: `pixi run -e <envname>`
        if let Some(env_name) = Self::detect_pixi(shellcmd) {
            return Self {
                env_type: EnvType::Pixi,
                env_name: Some(env_name),
                image_url: None,
            };
        }

        // Check for conda environment (from metadata or shellcmd)
        if let Some(env) = conda_env {
            return Self {
                env_type: EnvType::Conda,
                env_name: Some(env.to_string()),
                image_url: None,
            };
        }

        // Check shellcmd for conda pattern: `conda run -n <envname>`
        if let Some(env_name) = Self::detect_conda(shellcmd) {
            return Self {
                env_type: EnvType::Conda,
                env_name: Some(env_name),
                image_url: None,
            };
        }

        // Default: direct execution
        Self {
            env_type: EnvType::Direct,
            env_name: None,
            image_url: None,
        }
    }

    /// Detect pixi environment from shell command.
    fn detect_pixi(shellcmd: &str) -> Option<String> {
        PIXI_ENV_RE
            .captures(shellcmd)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Detect conda environment from shell command.
    fn detect_conda(shellcmd: &str) -> Option<String> {
        CONDA_ENV_RE
            .captures(shellcmd)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Detect container from shell command.
    fn detect_container(shellcmd: &str) -> Option<String> {
        CONTAINER_RE.captures(shellcmd).and_then(|caps| {
            // Try first capture group (singularity/apptainer), then second (docker)
            caps.get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().to_string())
        })
    }

    /// Get a display string for the environment.
    pub fn display(&self) -> String {
        match &self.env_type {
            EnvType::Pixi => {
                if let Some(name) = &self.env_name {
                    format!("pixi:{}", name)
                } else {
                    "pixi".to_string()
                }
            }
            EnvType::Conda => {
                if let Some(name) = &self.env_name {
                    format!("conda:{}", name)
                } else {
                    "conda".to_string()
                }
            }
            EnvType::Container => {
                if let Some(url) = &self.image_url {
                    // Truncate long URLs
                    if url.len() > 40 {
                        format!("container:...{}", &url[url.len() - 35..])
                    } else {
                        format!("container:{}", url)
                    }
                } else {
                    "container".to_string()
                }
            }
            EnvType::Direct => "direct".to_string(),
        }
    }
}

/// Pipeline error type classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineErrorType {
    /// Missing input file(s)
    MissingInput,
    /// Shell command failed with exit code
    CommandFailed,
    /// Rule exception
    RuleError,
    /// Workflow-level error
    WorkflowError,
    /// Directory locked by another process
    Locked,
    /// Incomplete output files
    IncompleteFiles,
    /// Syntax error in Snakefile
    SyntaxError,
    /// Generic/unclassified error
    Generic,
}

/// Structured pipeline error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineError {
    /// Error type classification
    pub error_type: PipelineErrorType,
    /// Rule name (if applicable)
    pub rule: Option<String>,
    /// Primary error message
    pub message: String,
    /// Additional details (file paths, exit codes, etc.)
    pub details: Vec<String>,
    /// Exit code (for command failures)
    pub exit_code: Option<i32>,
}

impl PipelineError {
    /// Create a new pipeline error.
    pub fn new(error_type: PipelineErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            rule: None,
            message: message.into(),
            details: Vec::new(),
            exit_code: None,
        }
    }

    /// Add a rule name.
    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }

    /// Add detail.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    /// Add exit code.
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Get icon for error type.
    pub fn icon(&self) -> &'static str {
        match self.error_type {
            PipelineErrorType::MissingInput => "📁",
            PipelineErrorType::CommandFailed => "💥",
            PipelineErrorType::RuleError => "📋",
            PipelineErrorType::WorkflowError => "⚙️",
            PipelineErrorType::Locked => "🔒",
            PipelineErrorType::IncompleteFiles => "⚠️",
            PipelineErrorType::SyntaxError => "📝",
            PipelineErrorType::Generic => "❌",
        }
    }

    /// Get short label for error type.
    pub fn label(&self) -> &'static str {
        match self.error_type {
            PipelineErrorType::MissingInput => "Missing Input",
            PipelineErrorType::CommandFailed => "Command Failed",
            PipelineErrorType::RuleError => "Rule Error",
            PipelineErrorType::WorkflowError => "Workflow Error",
            PipelineErrorType::Locked => "Locked",
            PipelineErrorType::IncompleteFiles => "Incomplete",
            PipelineErrorType::SyntaxError => "Syntax Error",
            PipelineErrorType::Generic => "Error",
        }
    }
}

/// Data source flags.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataSources {
    pub has_snakemake_metadata: bool,
    pub has_slurm_squeue: bool,
    pub has_slurm_sacct: bool,
    pub has_lsf_bjobs: bool,
    pub has_lsf_bhist: bool,
}

/// Unified job combining SLURM and snakemake data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique identifier
    pub id: String,

    /// Snakemake rule name
    pub rule: String,

    /// Wildcards as key=value string
    pub wildcards: Option<String>,

    /// Output file(s)
    pub outputs: Vec<String>,

    /// Input files
    pub inputs: Vec<String>,

    /// Current status
    pub status: JobStatus,

    /// Scheduler job ID (SLURM or LSF job ID, if submitted)
    pub scheduler_job_id: Option<String>,

    /// Shell command
    pub shellcmd: String,

    /// Timing information
    pub timing: JobTiming,

    /// Resource allocation (requested)
    pub resources: JobResources,

    /// Actual resource usage (for finished jobs)
    pub usage: Option<ResourceUsage>,

    /// Log file paths
    pub log_files: Vec<String>,

    /// Error details (if failed)
    pub error: Option<JobError>,

    /// Conda environment (from snakemake metadata)
    pub conda_env: Option<String>,

    /// Container image URL (from snakemake metadata)
    pub container_img_url: Option<String>,

    /// Data sources
    pub data_sources: DataSources,

    /// Whether this is a target rule (no outputs, like "all")
    pub is_target: bool,
}

/// Pipeline-level state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Snakemake run UUID
    pub run_uuid: Option<String>,

    /// Pipeline working directory
    pub working_dir: Utf8PathBuf,

    /// All jobs indexed by ID
    pub jobs: HashMap<String, Job>,

    /// Jobs grouped by rule
    pub jobs_by_rule: HashMap<String, Vec<String>>,

    /// Last update timestamp
    pub last_updated: DateTime<Utc>,

    /// Total jobs from snakemake log (if known)
    pub total_jobs: Option<usize>,

    /// Number of cores being used
    pub cores: Option<usize>,

    /// Host machine name
    pub host: Option<String>,

    /// Whether the pipeline has finished
    pub pipeline_finished: bool,

    /// Pipeline-level errors from main log (structured)
    pub pipeline_errors: Vec<PipelineError>,
}

impl PipelineState {
    pub fn new(working_dir: Utf8PathBuf) -> Self {
        Self {
            run_uuid: None,
            working_dir,
            jobs: HashMap::new(),
            jobs_by_rule: HashMap::new(),
            last_updated: Utc::now(),
            total_jobs: None,
            cores: None,
            host: None,
            pipeline_finished: false,
            pipeline_errors: Vec::new(),
        }
    }

    /// Update pipeline state from snakemake log info.
    pub fn update_from_log_info(&mut self, info: &charmer_core::SnakemakeLogInfo) {
        if info.total_jobs.is_some() {
            self.total_jobs = info.total_jobs;
        }
        if info.cores.is_some() {
            self.cores = info.cores;
        }
        if info.host.is_some() {
            self.host = info.host.clone();
        }
        self.pipeline_finished = info.finished;
        if !info.errors.is_empty() {
            self.pipeline_errors = info.errors.iter().map(|s| parse_error_string(s)).collect();
        }

        // Create synthetic jobs for target rules (rules without output files)
        // These rules appear in jobs_by_rule from the log but won't have metadata files
        // since they have no output files (e.g., "all" rule that just aggregates targets).
        // We detect these by parsing rule blocks in the log and checking for output: lines.
        for rule in &info.target_rules {
            if !self.jobs_by_rule.contains_key(rule) {
                let job_id = format!("__target_{}__", rule);
                let status = if info.finished && self.pipeline_errors.is_empty() {
                    JobStatus::Completed
                } else if info.finished {
                    JobStatus::Failed
                } else {
                    JobStatus::Pending
                };

                let job = Job {
                    id: job_id.clone(),
                    rule: rule.clone(),
                    wildcards: None,
                    outputs: Vec::new(),
                    inputs: Vec::new(),
                    status,
                    scheduler_job_id: None,
                    shellcmd: String::new(),
                    timing: JobTiming::default(),
                    resources: JobResources::default(),
                    usage: None,
                    log_files: Vec::new(),
                    error: None,
                    conda_env: None,
                    container_img_url: None,
                    data_sources: DataSources::default(),
                    is_target: true,
                };
                self.jobs.insert(job_id.clone(), job);
                self.jobs_by_rule.insert(rule.clone(), vec![job_id]);
            }
        }

        // Update status of existing target jobs based on pipeline state
        for rule in &info.target_rules {
            let job_id = format!("__target_{}__", rule);
            if let Some(job) = self.jobs.get_mut(&job_id) {
                if job.is_target {
                    job.status = if info.finished && self.pipeline_errors.is_empty() {
                        JobStatus::Completed
                    } else if info.finished {
                        JobStatus::Failed
                    } else {
                        JobStatus::Pending
                    };
                    // Update timing when pipeline finishes
                    if info.finished && job.timing.completed_at.is_none() {
                        job.timing.completed_at = Some(chrono::Utc::now());
                    }
                }
            }

            // Also update any regular jobs for this target rule (they have no outputs)
            if let Some(job_ids) = self.jobs_by_rule.get(rule).cloned() {
                for job_id in job_ids {
                    if let Some(job) = self.jobs.get_mut(&job_id) {
                        if job.outputs.is_empty() {
                            job.is_target = true;
                            job.status = if info.finished && self.pipeline_errors.is_empty() {
                                JobStatus::Completed
                            } else if info.finished {
                                JobStatus::Failed
                            } else {
                                JobStatus::Pending
                            };
                        }
                    }
                }
            }
        }
    }

    pub fn job_counts(&self) -> JobCounts {
        let mut counts = JobCounts::default();
        for job in self.jobs.values() {
            match job.status {
                JobStatus::Pending => counts.pending += 1,
                JobStatus::Queued => counts.queued += 1,
                JobStatus::Running => counts.running += 1,
                JobStatus::Completed => counts.completed += 1,
                JobStatus::Failed => counts.failed += 1,
                JobStatus::Cancelled => counts.cancelled += 1,
                JobStatus::Unknown => counts.unknown += 1,
            }
        }
        counts.total = self.jobs.len();
        counts
    }

    /// Estimate time remaining for the pipeline to complete.
    /// Returns (estimated_seconds, is_reliable) where is_reliable indicates
    /// if we have enough completed jobs to make a good estimate.
    pub fn estimate_eta(&self) -> Option<(u64, bool)> {
        let counts = self.job_counts();
        let total = self.total_jobs.unwrap_or(counts.total);

        // Need at least some completed jobs to estimate
        if counts.completed == 0 {
            return None;
        }

        // Calculate average runtime from completed jobs
        let mut total_runtime_secs: u64 = 0;
        let mut completed_with_timing = 0;

        for job in self.jobs.values() {
            if job.status == JobStatus::Completed {
                if let (Some(start), Some(end)) = (job.timing.started_at, job.timing.completed_at) {
                    let runtime = (end - start).num_seconds().max(0) as u64;
                    total_runtime_secs += runtime;
                    completed_with_timing += 1;
                }
            }
        }

        if completed_with_timing == 0 {
            return None;
        }

        let avg_runtime = total_runtime_secs / completed_with_timing as u64;

        // Calculate remaining work
        let remaining = total.saturating_sub(counts.completed);
        let running = counts.running;

        // Estimate for running jobs: average half their expected time remaining
        let running_contribution = if running > 0 {
            // Assume running jobs are on average halfway done
            (running as u64 * avg_runtime) / 2
        } else {
            0
        };

        // Estimate for pending jobs
        let pending_contribution = remaining.saturating_sub(running) as u64 * avg_runtime;

        // Total estimate (note: this assumes serial execution, actual time depends on parallelism)
        let estimate = running_contribution + pending_contribution;

        // Reliability: we have enough data if at least 20% of jobs are completed
        let is_reliable = counts.completed > 2 && (counts.completed * 5) >= total;

        Some((estimate, is_reliable))
    }

    /// Get ETA as a formatted string.
    pub fn eta_string(&self) -> Option<String> {
        self.estimate_eta().map(|(secs, reliable)| {
            let time_str = if secs >= 3600 {
                let hours = secs / 3600;
                let mins = (secs % 3600) / 60;
                format!("{}h{}m", hours, mins)
            } else if secs >= 60 {
                let mins = secs / 60;
                format!("{}m", mins)
            } else {
                format!("{}s", secs)
            };

            if reliable {
                format!("~{}", time_str)
            } else {
                format!("~{}?", time_str) // Add ? to indicate uncertainty
            }
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobCounts {
    pub total: usize,
    pub pending: usize,
    pub queued: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub unknown: usize,
}

/// Parse a raw error string into a structured PipelineError.
fn parse_error_string(error: &str) -> PipelineError {
    let error_lower = error.to_lowercase();

    // MissingInputException
    if error_lower.contains("missinginputexception") || error_lower.contains("missing input") {
        let mut pe = PipelineError::new(PipelineErrorType::MissingInput, error.to_string());
        // Try to extract rule name
        if let Some(rule) = extract_rule_from_error(error) {
            pe = pe.with_rule(rule);
        }
        // Try to extract file paths
        for line in error.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('/') || trimmed.contains("results/") || trimmed.contains("data/")
            {
                pe = pe.with_detail(trimmed.to_string());
            }
        }
        return pe;
    }

    // CalledProcessError / command failed
    if error_lower.contains("calledprocesserror")
        || error_lower.contains("error executing rule")
        || error_lower.contains("error in rule")
    {
        let mut pe = PipelineError::new(PipelineErrorType::CommandFailed, error.to_string());
        if let Some(rule) = extract_rule_from_error(error) {
            pe = pe.with_rule(rule);
        }
        // Try to extract exit code
        if let Some(code) = extract_exit_code(error) {
            pe = pe.with_exit_code(code);
        }
        return pe;
    }

    // Lock exception
    if error_lower.contains("lockexception") || error_lower.contains("directory cannot be locked") {
        return PipelineError::new(PipelineErrorType::Locked, error.to_string());
    }

    // Incomplete files
    if error_lower.contains("incompletefilesexception") || error_lower.contains("incomplete") {
        let mut pe = PipelineError::new(PipelineErrorType::IncompleteFiles, error.to_string());
        for line in error.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('/') || trimmed.contains("results/") {
                pe = pe.with_detail(trimmed.to_string());
            }
        }
        return pe;
    }

    // Syntax error
    if error_lower.contains("syntaxerror") || error_lower.contains("syntax error") {
        return PipelineError::new(PipelineErrorType::SyntaxError, error.to_string());
    }

    // Workflow error
    if error_lower.contains("workflowerror") || error_lower.contains("workflow error") {
        return PipelineError::new(PipelineErrorType::WorkflowError, error.to_string());
    }

    // Rule exception
    if error_lower.contains("ruleexception") {
        let mut pe = PipelineError::new(PipelineErrorType::RuleError, error.to_string());
        if let Some(rule) = extract_rule_from_error(error) {
            pe = pe.with_rule(rule);
        }
        return pe;
    }

    // Generic error
    let mut pe = PipelineError::new(PipelineErrorType::Generic, error.to_string());
    if let Some(rule) = extract_rule_from_error(error) {
        pe = pe.with_rule(rule);
    }
    pe
}

/// Extract rule name from error message.
fn extract_rule_from_error(error: &str) -> Option<String> {
    ERROR_RULE_RE
        .captures(error)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .filter(|rule| *rule != "the" && *rule != "a" && *rule != "an")
        .map(|s| s.to_string())
}

/// Extract exit code from error message.
fn extract_exit_code(error: &str) -> Option<i32> {
    EXIT_CODE_RE
        .captures(error)
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))
        .and_then(|m| m.as_str().parse().ok())
}
