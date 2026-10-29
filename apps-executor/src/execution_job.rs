use kube::{
    api::{Api, DeleteParams, PostParams, WatchEvent, WatchParams},
    Client, ResourceExt,
};
use kube::api::ObjectMeta;
use kube::core::{PodSpec, Container, Volume, VolumeMount};
use kube::core::ResourceRequirements;
use kube::core::SecurityContext;
use kube::core::Capabilities;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time::{timeout, sleep};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum JobExecutionError {
    #[error("Script must not be empty")]
    EmptyScript,
    #[error("Script exceeds the {limit} KiB limit", limit = super::execution::MAX_SCRIPT_BYTES / 1024)]
    ScriptTooLarge,
    #[error("Kubernetes client error: {0}")]
    KubeClient(String),
    #[error("Failed to create job: {0}")]
    JobCreation(String),
    #[error("Job timed out after {seconds}s", seconds = super::execution::DEFAULT_TIMEOUT.as_secs())]
    Timeout,
    #[error("Job failed: {0}")]
    JobFailed(String),
    #[error("Could not get job logs: {0}")]
    GetLogs(String),
    #[error("Could not parse grading results: {0}")]
    ParseGradingResults(String),
}

pub type Result<T> = std::result::Result<T, JobExecutionError>;

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

const MAX_SCRIPT_BYTES: usize = 16 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn runner_image() -> String {
    std::env::var("RUNNER_IMAGE").unwrap_or_else(|_| "powershell-runner:dev".to_string())
}

fn namespace() -> String {
    std::env::var("K8S_NAMESPACE").unwrap_or_else(|_| "powershell-learning".to_string())
}

async fn create_kube_client() -> Result<Client> {
    Client::try_default().await
        .map_err(|e| JobExecutionError::KubeClient(e.to_string()))
}

fn script_job_name() -> String {
    format!("ps-run-{}", Uuid::new_v4().simple())
}

fn grading_job_name(exercise_id: &str) -> String {
    format!("ps-grade-{}-{}", exercise_id, Uuid::new_v4().simple())
}

fn base_pod_spec(script: &str, command: Vec<String>, image: &str) -> PodSpec {
    let mut env = vec![
        kube::core::EnvVar {
            name: "SCRIPT".to_string(),
            value: Some(script.to_string()),
            value_from: None,
        },
    ];

    PodSpec {
        containers: vec![Container {
            name: "runner".to_string(),
            image: Some(image.to_string()),
            command: Some(command),
            env: Some(env),
            stdin: Some(true),
            stdin_once: Some(true),
            security_context: Some(SecurityContext {
                run_as_non_root: Some(true),
                run_as_user: Some(1000),
                read_only_root_filesystem: Some(true),
                allow_privilege_escalation: Some(false),
                capabilities: Some(Capabilities {
                    drop: Some(vec!["ALL".to_string()]),
                    add: None,
                }),
                ..Default::default()
            }),
            resources: Some(ResourceRequirements {
                limits: Some({
                    let mut m = BTreeMap::new();
                    m.insert("memory".to_string(), "64Mi".to_string());
                    m.insert("cpu".to_string(), "500m".to_string());
                    m
                }),
                requests: Some({
                    let mut m = BTreeMap::new();
                    m.insert("memory".to_string(), "32Mi".to_string());
                    m.insert("cpu".to_string(), "100m".to_string());
                    m
                }),
                ..Default::default()
            }),
            volume_mounts: Some(vec![
                VolumeMount {
                    name: "tmp-volume".to_string(),
                    mount_path: "/tmp".to_string(),
                    read_only: Some(false),
                    ..Default::default()
                }
            ]),
            ..Default::default()
        }],
        restart_policy: Some("Never".to_string()),
        volumes: Some(vec![
            Volume {
                name: "tmp-volume".to_string(),
                empty_dir: Some(kube::core::EmptyDirVolumeSource {
                    medium: Some("Memory".to_string()),
                    size_limit: Some("16Mi".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }
        ]),
        security_context: Some(kube::core::PodSecurityContext {
            run_as_non_root: Some(true),
            run_as_user: Some(1000),
            fs_group: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    }
}

async fn wait_for_job_completion(
    client: &Client,
    namespace: &str,
    job_name: &str,
    timeout_duration: Duration,
) -> Result<(bool, Option<i32>)> {
    let jobs: Api<kube::core::Job> = Api::namespaced(client.clone(), namespace);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout_duration {
            return Err(JobExecutionError::Timeout);
        }

        let job = jobs.get(job_name).await
            .map_err(|e| JobExecutionError::KubeClient(e.to_string()))?;

        if let Some(status) = job.status.as_ref() {
            if let Some(conditions) = status.conditions.as_ref() {
                for condition in conditions {
                    if condition.type_ == "Complete" && condition.status == "True" {
                        return Ok((false, Some(0)));
                    }
                    if condition.type_ == "Failed" && condition.status == "True" {
                        return Ok((false, Some(1)));
                    }
                }
            }

            if let Some(active) = status.active {
                if active == 0 && status.succeeded == Some(0) && status.failed.is_some() {
                    return Ok((false, Some(1)));
                }
            }
        }

        sleep(POLL_INTERVAL).await;
    }
}

async fn get_job_logs(client: &Client, namespace: &str, job_name: &str) -> Result<(String, String)> {
    let pods: Api<kube::core::Pod> = Api::namespaced(client.clone(), namespace);
    let label_selector = format!("job-name={}", job_name);

    let pod_list = pods.list(&kube::api::ListParams::default().labels(&label_selector)).await
        .map_err(|e| JobExecutionError::KubeClient(e.to_string()))?;

    let pod = pod_list.items.first().ok_or_else(|| {
        JobExecutionError::GetLogs("No pod found for job".to_string())
    })?;

    let pod_name = pod.name_any();

    let log_params = kube::api::LogParams {
        container: Some("runner".to_string()),
        follow: Some(false),
        previous: Some(false),
        timestamps: Some(false),
        tail_lines: Some(1000),
        ..Default::default()
    };

    let logs = pods.logs(&pod_name, &log_params).await
        .map_err(|e| JobExecutionError::GetLogs(e.to_string()))?;

    // In a real implementation, you'd separate stdout/stderr
    // For now, return all logs as stdout
    Ok((logs, String::new()))
}

pub async fn run_script_k8s(script: String) -> Result<RunResult> {
    if script.trim().is_empty() {
        return Err(JobExecutionError::EmptyScript);
    }

    if script.len() > MAX_SCRIPT_BYTES {
        return Err(JobExecutionError::ScriptTooLarge);
    }

    let client = create_kube_client().await?;
    let ns = namespace();
    let job_name = script_job_name();
    let image = runner_image();

    let pod_spec = base_pod_spec(
        &script,
        vec![
            "pwsh".to_string(),
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "-".to_string(),
        ],
        &image,
    );

    let job = kube::core::Job {
        metadata: ObjectMeta {
            name: Some(job_name.clone()),
            namespace: Some(ns.clone()),
            ..Default::default()
        },
        spec: Some(kube::core::JobSpec {
            template: kube::core::PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some({
                        let mut m = BTreeMap::new();
                        m.insert("app".to_string(), "powershell-runner".to_string());
                        m
                    }),
                    ..Default::default()
                }),
                spec: Some(pod_spec),
            },
            ttl_seconds_after_finished: Some(60),
            backoff_limit: Some(0),
            ..Default::default()
        }),
        ..Default::default()
    };

    let jobs: Api<kube::core::Job> = Api::namespaced(client.clone(), &ns);
    jobs.create(&PostParams::default(), &job).await
        .map_err(|e| JobExecutionError::JobCreation(e.to_string()))?;

    info!("Created job {}", job_name);

    let (timed_out, exit_code) = wait_for_job_completion(&client, &ns, &job_name, DEFAULT_TIMEOUT).await?;

    let (stdout, stderr) = get_job_logs(&client, &ns, &job_name).await?;

    // Clean up
    let _ = jobs.delete(&job_name, &DeleteParams::default()).await;

    Ok(RunResult {
        stdout,
        stderr,
        exit_code,
        timed_out,
    })
}

const PESTER_COMMAND: &str = r#"
$ErrorActionPreference = 'Stop'
Import-Module Pester -RequiredVersion 5.7.1

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

pub async fn grade_exercise_k8s(
    exercise_id: String,
    script: String,
    workspace_path: String,
    grading_path: String,
) -> Result<GradeResult> {
    if script.trim().is_empty() {
        return Err(JobExecutionError::EmptyScript);
    }

    if script.len() > MAX_SCRIPT_BYTES {
        return Err(JobExecutionError::ScriptTooLarge);
    }

    let client = create_kube_client().await?;
    let ns = namespace();
    let job_name = grading_job_name(&exercise_id);
    let image = runner_image();

    // For K8s Jobs, we mount the workspace and grading as ConfigMaps or emptyDir with init containers
    // For simplicity, we'll embed the script and test file as environment variables
    // In production, you'd use ConfigMaps or persistent volumes

    let pod_spec = base_pod_spec(
        &script,
        vec![
            "pwsh".to_string(),
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            PESTER_COMMAND.to_string(),
        ],
        &image,
    );

    // Add grading path volume mount
    // This would need a proper volume setup in production

    let job = kube::core::Job {
        metadata: ObjectMeta {
            name: Some(job_name.clone()),
            namespace: Some(ns.clone()),
            ..Default::default()
        },
        spec: Some(kube::core::JobSpec {
            template: kube::core::PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some({
                        let mut m = BTreeMap::new();
                        m.insert("app".to_string(), "powershell-grader".to_string());
                        m
                    }),
                    ..Default::default()
                }),
                spec: Some(pod_spec),
            },
            ttl_seconds_after_finished: Some(60),
            backoff_limit: Some(0),
            ..Default::default()
        }),
        ..Default::default()
    };

    let jobs: Api<kube::core::Job> = Api::namespaced(client.clone(), &ns);
    jobs.create(&PostParams::default(), &job).await
        .map_err(|e| JobExecutionError::JobCreation(e.to_string()))?;

    info!("Created grading job {}", job_name);

    let (timed_out, exit_code) = wait_for_job_completion(&client, &ns, &job_name, DEFAULT_TIMEOUT).await?;

    let (stdout, stderr) = get_job_logs(&client, &ns, &job_name).await?;

    // Clean up
    let _ = jobs.delete(&job_name, &DeleteParams::default()).await;

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| JobExecutionError::ParseGradingResults(e.to_string()))?;

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
        stderr,
        exit_code,
        timed_out,
    })
}