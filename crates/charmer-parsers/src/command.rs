//! Command execution utilities for scheduler queries.

use thiserror::Error;
use tokio::process::Command;

/// Error type for command execution.
#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Failed to execute {command}: {error}")]
    Execution { command: String, error: String },
    #[error("Command {command} failed: {stderr}")]
    Failed { command: String, stderr: String },
}

/// Execute a command and return stdout as a string.
///
/// This is a convenience wrapper that handles common error cases
/// and UTF-8 conversion for scheduler command output.
pub async fn run_command(cmd: &mut Command, name: &str) -> Result<String, CommandError> {
    let output = cmd.output().await.map_err(|e| CommandError::Execution {
        command: name.to_string(),
        error: e.to_string(),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CommandError::Failed {
            command: name.to_string(),
            stderr: stderr.to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Execute a command and return stdout, treating non-zero exit as OK.
///
/// Some commands (like bjobs with no jobs) return non-zero but are still valid.
pub async fn run_command_allow_failure(
    cmd: &mut Command,
    name: &str,
) -> Result<String, CommandError> {
    let output = cmd.output().await.map_err(|e| CommandError::Execution {
        command: name.to_string(),
        error: e.to_string(),
    })?;

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_success() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let result = run_command(&mut cmd, "echo").await.unwrap();
        assert_eq!(result.trim(), "hello");
    }

    #[tokio::test]
    async fn test_run_command_not_found() {
        let mut cmd = Command::new("nonexistent_command_12345");
        let result = run_command(&mut cmd, "nonexistent").await;
        assert!(matches!(result, Err(CommandError::Execution { .. })));
    }
}
