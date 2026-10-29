use axum::{
    Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

mod executor_client;
use executor_client::ExecutorClientWrapper;

const MAX_SCRIPT_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
struct RunRequest {
    script: String,
}

#[derive(Serialize)]
struct RunResponse {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
}

#[derive(Serialize, Deserialize)]
struct Exercise {
    id: String,
    title: String,
    description: String,
    instructions: String,
}

#[derive(Deserialize)]
struct ExerciseSubmission {
    script: String,
}

#[derive(Serialize)]
struct ExerciseResult {
    exercise_id: String,
    passed: bool,
    message: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
}

#[derive(Clone)]
struct AppState {
    executor_client: Arc<tokio::sync::Mutex<ExecutorClientWrapper>>,
    exercises_dir: PathBuf,
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn run_powershell(
    State(state): State<AppState>,
    Json(request): Json<RunRequest>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    if request.script.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Script must not be empty".into()));
    }

    if request.script.len() > MAX_SCRIPT_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Script exceeds the 16 KiB limit".into(),
        ));
    }

    let mut client = state.executor_client.lock().await;
    let result = client.run_script(request.script).await.map_err(|e| {
        error!("Executor run_script failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Execution failed: {}", e),
        )
    })?;

    Ok(Json(RunResponse {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_code: result.exit_code,
        timed_out: result.timed_out,
    }))
}

async fn submit_exercise(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ExerciseSubmission>,
) -> Result<Json<ExerciseResult>, (StatusCode, String)> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err((StatusCode::NOT_FOUND, "Exercise not found".into()));
    }

    if request.script.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Script must not be empty".into()));
    }

    if request.script.len() > MAX_SCRIPT_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "Script is too large".into()));
    }

    let exercise_dir = state.exercises_dir.join(&id);
    let exercise_file = exercise_dir.join("exercise.json");
    let tests_file = exercise_dir.join("tests.Tests.ps1");

    let contents = tokio::fs::read_to_string(&exercise_file)
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                (StatusCode::NOT_FOUND, "Exercise not found".into())
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to read exercise definition".into(),
                )
            }
        })?;

    let exercise: Exercise = serde_json::from_str(&contents).map_err(|error| {
        error!("Invalid exercise definition for {id}: {error}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid exercise definition".into(),
        )
    })?;

    if exercise.id != id {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Exercise ID mismatch".into(),
        ));
    }

    if !tokio::fs::try_exists(&tests_file).await.unwrap_or(false) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Exercise grading tests are missing".into(),
        ));
    }

    // Create a temporary directory for the learner's script.
    let workspace = tempfile::tempdir().map_err(|error| {
        error!("Failed to create submission workspace: {error}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to prepare submission".into(),
        )
    })?;

    let learner_script = workspace.path().join("learner.ps1");

    tokio::fs::write(&learner_script, &request.script)
        .await
        .map_err(|error| {
            error!("Failed to write learner script: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to prepare submission".into(),
            )
        })?;

    let workspace_path = workspace.path().to_string_lossy().into_owned();

    let mut client = state.executor_client.lock().await;
    let grade_result = client
        .grade_exercise(
            exercise.id.clone(),
            request.script,
            workspace_path,
            exercise_dir.to_string_lossy().into_owned(),
        )
        .await
        .map_err(|e| {
            error!("Executor grade_exercise failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Grading failed: {}", e),
            )
        })?;

    Ok(Json(ExerciseResult {
        exercise_id: exercise.id,
        passed: grade_result.passed,
        message: grade_result.message,
        stdout: String::new(),
        stderr: grade_result.stderr,
        exit_code: grade_result.exit_code,
        timed_out: grade_result.timed_out,
    }))
}

async fn list_exercises(State(state): State<AppState>) -> Json<Vec<Exercise>> {
    let mut exercises = Vec::new();

    let mut entries = match tokio::fs::read_dir(&state.exercises_dir).await {
        Ok(entries) => entries,
        Err(error) => {
            error!("Failed to read exercises directory: {error}");
            return Json(exercises);
        }
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(error) => {
                error!("Failed to read exercise directory entry: {error}");
                break;
            }
        };

        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let exercise_file = path.join("exercise.json");

        let contents = match tokio::fs::read_to_string(&exercise_file).await {
            Ok(contents) => contents,
            Err(error) => {
                warn!("Skipping exercise at {}: {error}", exercise_file.display());
                continue;
            }
        };

        match serde_json::from_str::<Exercise>(&contents) {
            Ok(exercise) => exercises.push(exercise),
            Err(error) => {
                warn!(
                    "Skipping invalid exercise file {}: {error}",
                    exercise_file.display()
                );
            }
        }
    }

    exercises.sort_by(|a, b| a.id.cmp(&b.id));

    Json(exercises)
}

async fn get_exercise(Path(id): Path<String>) -> Result<Json<Exercise>, StatusCode> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(StatusCode::NOT_FOUND);
    }

    let exercises_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../content/exercises");
    let exercise_file = std::path::Path::new(exercises_dir)
        .join(&id)
        .join("exercise.json");

    let contents = match tokio::fs::read_to_string(&exercise_file).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(StatusCode::NOT_FOUND);
        }
        Err(error) => {
            error!(
                "Failed to read exercise {}: {error}",
                exercise_file.display()
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let exercise: Exercise = serde_json::from_str(&contents).map_err(|error| {
        error!("Invalid exercise file {}: {error}", exercise_file.display());
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if exercise.id != id {
        error!("Exercise ID mismatch in {}", exercise_file.display());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(exercise))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,apps_api=debug,axum=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let executor_addr =
        std::env::var("EXECUTOR_ADDR").unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let exercises_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/exercises");

    info!("Connecting to executor at {}", executor_addr);
    let mut executor_client = ExecutorClientWrapper::connect(&executor_addr).await?;

    // Test connection
    match executor_client.health().await {
        Ok(health) => info!(
            "Executor health check: {} v{}",
            health.status, health.version
        ),
        Err(e) => warn!("Executor health check failed: {}", e),
    }

    let state = AppState {
        executor_client: Arc::new(tokio::sync::Mutex::new(executor_client)),
        exercises_dir,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/run", post(run_powershell))
        .route("/api/exercises", get(list_exercises))
        .route("/api/exercises/{id}", get(get_exercise))
        .route("/api/exercises/{id}/submit", post(submit_exercise))
        .layer(DefaultBodyLimit::max(20 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;

    let listener = TcpListener::bind(addr).await?;
    info!("API listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
