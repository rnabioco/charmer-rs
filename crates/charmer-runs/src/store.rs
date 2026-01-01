use crate::types::RunsState;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Persistent storage for run state.
pub struct RunStore {
    path: Utf8PathBuf,
}

impl RunStore {
    /// Create a store for the given working directory.
    ///
    /// State is stored at `.snakemake/charmer/runs.json` within the working directory.
    pub fn new(working_dir: &Utf8Path) -> Self {
        let path = working_dir
            .join(".snakemake")
            .join("charmer")
            .join("runs.json");
        Self { path }
    }

    /// Get the path to the state file.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Load runs state from disk.
    ///
    /// Returns an empty state if the file doesn't exist.
    pub fn load(&self) -> Result<RunsState, StoreError> {
        if !self.path.exists() {
            return Ok(RunsState::default());
        }
        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Save runs state to disk.
    ///
    /// Creates parent directories if needed.
    pub fn save(&self, state: &RunsState) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(state)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RunInfo;
    use tempfile::TempDir;

    #[test]
    fn test_store_load_nonexistent() {
        let temp = TempDir::new().unwrap();
        let store = RunStore::new(Utf8Path::from_path(temp.path()).unwrap());
        let state = store.load().unwrap();
        assert!(state.runs.is_empty());
    }

    #[test]
    fn test_store_save_and_load() {
        let temp = TempDir::new().unwrap();
        let working_dir = Utf8Path::from_path(temp.path()).unwrap();
        let store = RunStore::new(working_dir);

        let mut state = RunsState::default();
        state.upsert_run(RunInfo::new("test-run".to_string(), working_dir.to_path_buf()));

        store.save(&state).unwrap();
        assert!(store.path().exists());

        let loaded = store.load().unwrap();
        assert_eq!(loaded.runs.len(), 1);
        assert_eq!(loaded.runs[0].run_uuid, "test-run");
    }
}
