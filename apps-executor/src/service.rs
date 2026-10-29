use executor::{
    GradeRequest, GradeResponse, HealthRequest, HealthResponse, RunRequest, RunResponse,
    executor_server::Executor,
};
use tonic::{Request, Response, Status};
use tracing::{debug, info};

use crate::execution::{RunConfig, grade_exercise, health_check, run_script};

pub struct ExecutorService {
    config: RunConfig,
}

impl ExecutorService {
    pub fn new() -> Self {
        Self {
            config: RunConfig::default(),
        }
    }
}

impl Default for ExecutorService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl Executor for ExecutorService {
    async fn run_script(
        &self,
        request: Request<RunRequest>,
    ) -> Result<Response<RunResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "RunScript request: script_len={}, timeout={}s",
            req.script.len(),
            req.timeout_seconds
        );

        let mut config = self.config.clone();
        if req.timeout_seconds > 0 {
            config.timeout = std::time::Duration::from_secs(req.timeout_seconds as u64);
        }
        if req.max_output_bytes > 0 {
            config.max_output_bytes = req.max_output_bytes as usize;
        }

        let result = run_script(req.script, config)
            .await
            .map_err(|e| Status::internal(format!("Execution failed: {}", e)))?;

        Ok(Response::new(RunResponse {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code.unwrap_or(-1),
            timed_out: result.timed_out,
        }))
    }

    async fn grade_exercise(
        &self,
        request: Request<GradeRequest>,
    ) -> Result<Response<GradeResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "GradeExercise request: exercise_id={}, script_len={}",
            req.exercise_id,
            req.script.len()
        );

        let config = self.config.clone();

        let result = grade_exercise(
            req.exercise_id,
            req.script,
            req.workspace_path,
            req.grading_path,
            config,
        )
        .await
        .map_err(|e| Status::internal(format!("Grading failed: {}", e)))?;

        Ok(Response::new(GradeResponse {
            passed: result.passed,
            message: result.message,
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code.unwrap_or(-1),
            timed_out: result.timed_out,
        }))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let (status, version) = health_check();
        info!("Health check requested");
        Ok(Response::new(HealthResponse { status, version }))
    }
}
