//! TUI components.

pub mod footer;
pub mod header;
pub mod job_detail;
pub mod job_list;
pub mod log_viewer;
pub mod rule_summary;
pub mod view_tabs;

pub use footer::Footer;
pub use header::Header;
pub use job_detail::JobDetail;
pub use job_list::{ChainPosition, DepRelation, DependencyCache, JobList, compute_dependencies};
pub use log_viewer::{LogViewer, LogViewerState};
pub use rule_summary::RuleSummary;
pub use view_tabs::ViewTabs;
