use executor::executor_client::ExecutorClient;
use executor::{GradeRequest, RunRequest};
use std::time::Duration;
use thiserror::Error;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info};

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ExecutorClientError {
    #[error("Failed to connect to executor: {0}")]
    Connection(String),
    #[error("Executor returned error: {0}")]
    Grpc(String),
    #[error("Executor timed out")]
    Timeout,
}

pub struct ExecutorClientWrapper {
    client: ExecutorClient<Channel>,
    default_timeout: Duration,
}

impl ExecutorClientWrapper {
    pub async fn connect(addr: &str) -> Result<Self, ExecutorClientError> {
        let channel = Endpoint::from_shared(addr.to_string())
            .map_err(|e| ExecutorClientError::Connection(e.to_string()))?
            .connect()
            .await
            .map_err(|e| ExecutorClientError::Connection(e.to_string()))?;

        let client = ExecutorClient::new(channel);

        info!("Connected to executor at {}", addr);

        Ok(Self {
            client,
            default_timeout: Duration::from_secs(10),
        })
    }

    pub async fn run_script(&mut self, script: String) -> Result<RunResult, ExecutorClientError> {
        debug!("Sending RunScript request, script_len={}", script.len());

        let request = RunRequest {
            script,
            timeout_seconds: self.default_timeout.as_secs() as i32,
            max_output_bytes: 64 * 1024,
        };

        let response = self
            .client
            .run_script(request)
            .await
            .map_err(|e| ExecutorClientError::Grpc(e.to_string()))?;

        let resp = response.into_inner();
        Ok(RunResult {
            stdout: resp.stdout,
            stderr: resp.stderr,
            exit_code: if resp.exit_code >= 0 { Some(resp.exit_code) } else { None },
            timed_out: resp.timed_out,
        })
    }

    pub async fn grade_exercise(
        &mut self,
        exercise_id: String,
        script: String,
        workspace_path: String,
        grading_path: String,
    ) -> Result<GradeResult, ExecutorClientError> {
        debug!(
            "Sending GradeExercise request: exercise_id={}, script_len={}",
            exercise_id,
            script.len()
        );

        let request = GradeRequest {
            exercise_id,
            script,
            workspace_path,
            grading_path,
        };

        let response = self
            .client
            .grade_exercise(request)
            .await
            .map_err(|e| ExecutorClientError::Grpc(e.to_string()))?;

        let resp = response.into_inner();
        Ok(GradeResult {
            passed: resp.passed,
            message: resp.message,
            stdout: resp.stdout,
            stderr: resp.stderr,
            exit_code: if resp.exit_code >= 0 { Some(resp.exit_code) } else { None },
            timed_out: resp.timed_out,
        })
    }

    pub async fn health(&mut self) -> Result<HealthResult, ExecutorClientError> {
        let request = executor::HealthRequest {};
        let response = self
            .client
            .health(request)
            .await
            .map_err(|e| ExecutorClientError::Grpc(e.to_string()))?;

        let resp = response.into_inner();
        Ok(HealthResult {
            status: resp.status,
            version: resp.version,
        })
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthResult {
    pub status: String,
    pub version: String,
}