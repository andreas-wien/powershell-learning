use std::net::SocketAddr;
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod execution;
mod service;

use service::ExecutorService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,apps_executor=debug,tonic=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = std::env::var("EXECUTOR_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    let executor_service = ExecutorService::new();
    let svc = executor::executor_server::ExecutorServer::new(executor_service);

    tracing::info!("Starting Executor gRPC server on {}", addr);

    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}
