use std::{io, process::Stdio, time::Duration};
use tempfile::tempdir;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};
use thiserror::Error;

const MAX_SCRIPT_BYTES: usize = 16 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ExecutionError {
    #[error("Script must not be empty")]
    EmptyScript,
    #[error("Script exceeds the {limit} KiB limit", limit = MAX_SCRIPT_BYTES / 1024)]
    ScriptTooLarge,
    #[error("Could not start container: {0}")]
    ContainerStart(String),
    #[error("Could not write script to container: {0}")]
    WriteScript(String),
    #[error("Could not wait for container: {0}")]
    WaitContainer(String),
    #[error("Container timed out after {seconds}s", seconds = DEFAULT_TIMEOUT.as_secs())]
    Timeout,
    #[error("Could not read stdout: {0}")]
    ReadStdout(String),
    #[error("Could not read stderr: {0}")]
    ReadStderr(String),
    #[error("Invalid UTF-8 in output: {0}")]
    InvalidUtf8(String),
    #[error("Grading tests not found at {path}")]
    GradingTestsMissing { path: String },
    #[error("Failed to create workspace: {0}")]
    WorkspaceCreation(String),
    #[error("Failed to write learner script: {0}")]
    WriteLearnerScript(String),
    #[error("Could not parse grading results: {0}")]
    ParseGradingResults(String),
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub image: String,
    pub timeout: Duration,
    pub max_script_bytes: usize,
    pub max_output_bytes: usize,
    pub tmpfs_size: String,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            image: std::env::var("RUNNER_IMAGE").unwrap_or_else(|_| "powershell-runner:dev".to_string()),
            timeout: DEFAULT_TIMEOUT,
            max_script_bytes: MAX_SCRIPT_BYTES,
            max_output_bytes: MAX_OUTPUT_BYTES,
            tmpfs_size: "16m".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GradeResult {
    pub passed: bool,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

async fn read_capped<R: AsyncRead + Unpin>(mut reader: R, limit: usize) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 4096];

    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }

        let remaining = limit.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..count.min(remaining)]);
    }

    Ok(output)
}

/// Build docker run arguments in correct order: options -> volumes -> image -> command
fn build_docker_args(config: &RunConfig, extra_volumes: &[String]) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network=none".to_string(),
        "--read-only".to_string(),
        format!("--tmpfs=/tmp:rw,noexec,nosuid,size={}", config.tmpfs_size),
        "--cap-drop=ALL".to_string(),
        "--security-opt=no-new-privileges".to_string(),
        "--env".to_string(),
        "TERM=dumb".to_string(),
    ];
    args.extend(extra_volumes.iter().cloned());
    args.push(config.image.clone());
    args
}

/// Convert Windows path to Docker-compatible path on Windows (e.g., C:\foo -> /c/foo)
fn to_docker_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix("\\\\?\\") {
        to_docker_path(std::path::Path::new(stripped))
    } else if s.len() >= 3 && s.chars().nth(1) == Some(':') && s.chars().nth(2) == Some('\\') {
        let drive = s.chars().next().unwrap().to_lowercase().to_string();
        let rest = &s[3..].replace('\\', "/");
        format!("/{}/{}", drive, rest)
    } else {
        s.replace('\\', "/")
    }
}

pub async fn run_script(script: String, config: RunConfig) -> Result<RunResult, ExecutionError> {
    if script.trim().is_empty() {
        return Err(ExecutionError::EmptyScript);
    }

    if script.len() > config.max_script_bytes {
        return Err(ExecutionError::ScriptTooLarge);
    }

    let docker_args = build_docker_args(&config, &[]);
    let mut child = Command::new("docker")
        .args(&docker_args)
        .args(["-i", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ExecutionError::ContainerStart(e.to_string()))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ExecutionError::ContainerStart("Missing stdin".into()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ExecutionError::ContainerStart("Missing stdout".into()))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ExecutionError::ContainerStart("Missing stderr".into()))?;

    let stdout_task = tokio::spawn(read_capped(stdout, config.max_output_bytes));
    let stderr_task = tokio::spawn(read_capped(stderr, config.max_output_bytes));

    if let Err(e) = stdin.write_all(script.as_bytes()).await {
        let _ = child.kill().await;
        return Err(ExecutionError::WriteScript(e.to_string()));
    }
    drop(stdin);

    let (timed_out, exit_code) = match timeout(config.timeout, child.wait()).await {
        Ok(Ok(status)) => (false, status.code()),
        Ok(Err(e)) => {
            return Err(ExecutionError::WaitContainer(e.to_string()));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            (true, None)
        }
    };

    let stdout = stdout_task
        .await
        .map_err(|e| ExecutionError::ReadStdout(e.to_string()))?
        .map_err(|e| ExecutionError::ReadStdout(e.to_string()))?;

    let stderr = stderr_task
        .await
        .map_err(|e| ExecutionError::ReadStderr(e.to_string()))?
        .map_err(|e| ExecutionError::ReadStderr(e.to_string()))?;

    Ok(RunResult {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code,
        timed_out,
    })
}

const PESTER_COMMAND: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Import-Module Pester -RequiredVersion 5.7.1 | Out-Null

$result = Invoke-Pester `
    -Path '/grading/tests.Tests.ps1' `
    -PassThru `
    -Output None

$testResults = @(
    $result.Tests | ForEach-Object {
        $message = ''
        if ($null -ne $_.ErrorRecord) {
            $message = $_.ErrorRecord.ToString()
        }

        [pscustomobject]@{
            name    = $_.Name
            result  = [string]$_.Result
            message = $message
        }
    }
)

[pscustomobject]@{
    passed       = ($result.TotalCount -gt 0 -and $result.FailedCount -eq 0)
    passed_count = $result.PassedCount
    failed_count = $result.FailedCount
    total_count  = $result.TotalCount
    tests        = $testResults
} | ConvertTo-Json -Depth 8 -Compress
"#;

pub async fn grade_exercise(
    _exercise_id: String,
    script: String,
    workspace_path: String,
    grading_path: String,
    config: RunConfig,
) -> Result<GradeResult, ExecutionError> {
    if script.trim().is_empty() {
        return Err(ExecutionError::EmptyScript);
    }

    if script.len() > config.max_script_bytes {
        return Err(ExecutionError::ScriptTooLarge);
    }

    let _workspace_path = tokio::fs::canonicalize(&workspace_path)
        .await
        .map_err(|e| ExecutionError::ContainerStart(format!("Could not resolve workspace path: {e}")))?;

    let grading_path = tokio::fs::canonicalize(&grading_path)
        .await
        .map_err(|e| ExecutionError::ContainerStart(format!("Could not resolve grading path: {e}")))?;

    let tests_file = grading_path.join("tests.Tests.ps1");
    if !tokio::fs::try_exists(&tests_file).await.unwrap_or(false) {
        return Err(ExecutionError::GradingTestsMissing {
            path: tests_file.display().to_string(),
        });
    }

    let workspace = tempdir().map_err(|e| ExecutionError::WorkspaceCreation(e.to_string()))?;
    let learner_script = workspace.path().join("learner.ps1");

    tokio::fs::write(&learner_script, &script)
        .await
        .map_err(|e| ExecutionError::WriteLearnerScript(e.to_string()))?;

    let workspace_mount = format!("{}:/workspace:ro", to_docker_path(workspace.path()));
    let grading_mount = format!("{}:/grading:ro", to_docker_path(&grading_path));

    let extra_volumes = vec![
        "--volume".to_string(),
        workspace_mount,
        "--volume".to_string(),
        grading_mount,
    ];

    let docker_args = build_docker_args(&config, &extra_volumes);
    let mut child = Command::new("docker")
        .args(&docker_args)
        .args([
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            PESTER_COMMAND.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ExecutionError::ContainerStart(format!("Could not start Pester runner: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ExecutionError::ContainerStart("Missing runner stdout".into()))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ExecutionError::ContainerStart("Missing runner stderr".into()))?;

    let stdout_task = tokio::spawn(read_capped(stdout, config.max_output_bytes));
    let stderr_task = tokio::spawn(read_capped(stderr, config.max_output_bytes));

    let (timed_out, exit_code) = match timeout(config.timeout, child.wait()).await {
        Ok(Ok(status)) => (false, status.code()),
        Ok(Err(e)) => {
            return Err(ExecutionError::WaitContainer(format!("Could not wait for Pester runner: {e}")));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            (true, None)
        }
    };

    let stdout = stdout_task
        .await
        .map_err(|e| ExecutionError::ReadStdout(e.to_string()))?
        .map_err(|e| ExecutionError::ReadStdout(e.to_string()))?;

    let stderr = stderr_task
        .await
        .map_err(|e| ExecutionError::ReadStderr(e.to_string()))?
        .map_err(|e| ExecutionError::ReadStderr(e.to_string()))?;

    let run_result = RunResult {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code,
        timed_out,
    };

    let parsed: serde_json::Value = serde_json::from_str(&run_result.stdout)
        .map_err(|e| ExecutionError::ParseGradingResults(e.to_string()))?;

    let passed = parsed["passed"].as_bool().unwrap_or(false);
    let passed_count = parsed["passed_count"].as_u64().unwrap_or(0);
    let failed_count = parsed["failed_count"].as_u64().unwrap_or(0);
    let total_count = parsed["total_count"].as_u64().unwrap_or(0);

    let message = if passed {
        format!("Correct! All {total_count} tests passed.")
    } else {
        format!(
            "Not quite. {passed_count} of {total_count} tests passed; {failed_count} failed."
        )
    };

    Ok(GradeResult {
        passed,
        message,
        stdout: String::new(),
        stderr: run_result.stderr,
        exit_code: run_result.exit_code,
        timed_out: run_result.timed_out,
    })
}

pub fn health_check() -> (String, String) {
    ("ok".to_string(), env!("CARGO_PKG_VERSION").to_string())
}