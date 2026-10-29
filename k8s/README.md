# Kubernetes Deployment for PowerShell Learning Platform

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌──────────────────┐
│  Svelte     │────▶│  powershell-│────▶│  powershell-     │
│  Frontend   │     │  api        │     │  executor        │
└─────────────┘     └─────────────┘     └──────────────────┘
                           │                      │
                           ▼                      ▼
                    ┌─────────────┐        ┌──────────────────┐
                    │  Exercise   │        │  Podman /        │
                    │  Definitions│        │  K8s Jobs        │
                    └─────────────┘        └──────────────────┘
```

## Components

| Component | Port | Protocol | Purpose |
|-----------|------|----------|---------|
| powershell-api | 3000 | HTTP | REST API for frontend |
| powershell-executor | 50051 | gRPC | Script execution & grading |

## Prerequisites

- Kubernetes cluster (v1.28+)
- kubectl configured
- kustomize (built into kubectl)
- Container registry (Docker Hub, GHCR, etc.)
- Ingress controller (nginx-ingress recommended)
- cert-manager for TLS (optional)

## Build & Push Images

```bash
# Build executor
docker build -t your-registry/powershell-executor:latest -f apps-executor/Dockerfile .

# Build API
docker build -t your-registry/powershell-api:latest -f apps-api/Dockerfile .

# Push to registry
docker push your-registry/powershell-executor:latest
docker push your-registry/powershell-api:latest
```

## Deploy

```bash
# Update kustomization.yaml with your registry
# Edit the images section with your actual registry

# Deploy
kubectl apply -k k8s/

# Check status
kubectl get pods -n powershell-learning
kubectl get svc -n powershell-learning

# View logs
kubectl logs -n powershell-learning -l app=powershell-executor -f
kubectl logs -n powershell-learning -l app=powershell-api -f
```

## Configuration

### Environment Variables

#### Executor
| Variable | Default | Description |
|----------|---------|-------------|
| RUNNER_IMAGE | powershell-runner:dev | Container image for PowerShell execution |
| EXECUTOR_ADDR | 0.0.0.0:50051 | gRPC server address |
| RUST_LOG | info,apps_executor=debug | Logging level |

#### API
| Variable | Default | Description |
|----------|---------|-------------|
| EXECUTOR_ADDR | http://powershell-executor:50051 | Executor gRPC endpoint |
| API_ADDR | 0.0.0.0:3000 | HTTP server address |
| RUST_LOG | info,apps_api=debug | Logging level |

### Exercise Content

Exercises are loaded from the `content/exercises/` directory. Each exercise needs:

```
content/exercises/
├── exercise-id/
│   ├── exercise.json      # Exercise metadata
│   └── tests.Tests.ps1    # Pester tests
```

Example `exercise.json`:
```json
{
  "id": "hello-world",
  "title": "Hello World",
  "description": "Write a script that outputs 'Hello, World!'",
  "instructions": "Create a script that writes 'Hello, World!' to stdout."
}
```

Example `tests.Tests.ps1`:
```powershell
Describe "Hello World Exercise" {
    It "Outputs Hello, World!" {
        $result = & ./learner.ps1
        $result | Should -Be "Hello, World!"
    }
}
```

## Alternative: K8s Job-Based Execution

For better isolation without Podman-in-Podman, the executor can create Kubernetes Jobs instead. See `executor-job-deployment.yaml` for that approach.

## Monitoring

Both services expose Prometheus metrics:
- Executor: `:50051/metrics` (via grpc_health_probe)
- API: `:3000/metrics` (via tower-http trace)

## Scaling

```bash
# Scale executor (CPU/memory intensive)
kubectl scale deployment powershell-executor -n powershell-learning --replicas=5

# Scale API (stateless, lightweight)
kubectl scale deployment powershell-api -n powershell-learning --replicas=3
```

## Troubleshooting

### Executor can't start containers
- Ensure podman socket is mounted: `/run/podman/podman.sock`
- Check podman is running on nodes: `systemctl status podman`
- Verify runner image exists: `podman images | grep powershell-runner`

### API can't connect to executor
- Check service DNS: `nslookup powershell-executor.powershell-learning`
- Verify network policies allow gRPC traffic
- Check executor logs for errors

### Exercise grading fails
- Verify tests.Tests.ps1 exists in exercise directory
- Check Pester version in runner image (5.7.1)
- Ensure workspace/grading paths are correctly mounted