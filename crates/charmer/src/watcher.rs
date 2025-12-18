//! File watcher for snakemake metadata directory.

use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Events from the file watcher.
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    /// A metadata file was created or modified
    MetadataFile(Utf8PathBuf),
    /// The metadata directory was created
    MetadataDirectoryCreated,
    /// Watcher error
    Error(String),
}

/// File watcher for the metadata directory.
pub struct MetadataWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<WatcherEvent>,
    metadata_dir: Utf8PathBuf,
}

impl MetadataWatcher {
    /// Create a new metadata watcher for the given working directory.
    pub fn new(working_dir: &Utf8Path) -> Result<Self> {
        let metadata_dir = working_dir.join(".snakemake").join("metadata");

        let (tx, rx) = channel();

        // Create watcher
        let watcher = create_watcher(tx, metadata_dir.clone())?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            metadata_dir,
        })
    }

    /// Get the metadata directory path.
    #[allow(dead_code)]
    pub fn metadata_dir(&self) -> &Utf8Path {
        &self.metadata_dir
    }

    /// Try to receive an event with a timeout.
    #[allow(dead_code)]
    pub fn try_recv(&self, timeout: Duration) -> Option<WatcherEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Try to receive an event without blocking.
    pub fn try_recv_nonblocking(&self) -> Option<WatcherEvent> {
        self.receiver.try_recv().ok()
    }
}

/// Create and configure the file watcher.
fn create_watcher(
    tx: Sender<WatcherEvent>,
    metadata_dir: Utf8PathBuf,
) -> Result<RecommendedWatcher> {
    let tx_clone = tx.clone();
    let metadata_dir_clone = metadata_dir.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| match res {
            Ok(event) => {
                handle_event(event, &tx_clone, &metadata_dir_clone);
            }
            Err(e) => {
                let _ = tx_clone.send(WatcherEvent::Error(e.to_string()));
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .into_diagnostic()?;

    // Try to watch the metadata directory if it exists
    if metadata_dir.exists() {
        watcher
            .watch(metadata_dir.as_std_path(), RecursiveMode::NonRecursive)
            .into_diagnostic()?;
    } else {
        // Watch the .snakemake directory to detect when metadata/ is created
        let snakemake_dir = metadata_dir
            .parent()
            .unwrap_or_else(|| Utf8Path::new(".snakemake"));
        if snakemake_dir.exists() {
            watcher
                .watch(snakemake_dir.as_std_path(), RecursiveMode::NonRecursive)
                .into_diagnostic()?;
        }
        // If .snakemake doesn't exist either, watcher won't send events until created
    }

    Ok(watcher)
}

/// Handle a file system event.
fn handle_event(event: Event, tx: &Sender<WatcherEvent>, metadata_dir: &Utf8Path) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in event.paths {
                let path = match Utf8PathBuf::try_from(path) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                // Check if this is the metadata directory being created
                if path == metadata_dir {
                    let _ = tx.send(WatcherEvent::MetadataDirectoryCreated);
                    continue;
                }

                // Check if this is a metadata file
                if path.starts_with(metadata_dir) && path.is_file() {
                    // Skip hidden files
                    if let Some(name) = path.file_name() {
                        if name.starts_with('.') {
                            continue;
                        }
                    }

                    let _ = tx.send(WatcherEvent::MetadataFile(path));
                }
            }
        }
        _ => {
            // Ignore other event types (access, remove, etc.)
        }
    }
}
