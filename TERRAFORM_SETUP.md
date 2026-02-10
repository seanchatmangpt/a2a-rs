# A2A-RS Terraform Infrastructure Setup

Complete Terraform configuration for deploying A2A-RS services to Google Cloud Run.

## Overview

The `terraform/` directory contains production-ready Infrastructure-as-Code for deploying three microservices:

1. **osiris-edge** - HTTP/WebSocket server with admissions control and WIP gate
2. **osiris-compiler** - Deterministic compilation with 7-stage pipeline
3. **osiris-marketplace** - Google Workspace integrations and Cloud Service Control

## Quick Start

Get services running in 10 minutes:

```bash
cd terraform
terraform init
cp terraform.tfvars.dev terraform.tfvars
# Edit: terraform.tfvars - set project_id
terraform apply -var-file="terraform.tfvars"
```

## What's Included

### Core Terraform Files

| File | Size | Purpose |
|------|------|---------|
| `main.tf` | 21 KB | Cloud Run services, IAM, VPC, networking |
| `variables.tf` | 14 KB | 40+ input variables with validation |
| `outputs.tf` | 11 KB | Service endpoints, accounts, monitoring |
| `provider.tf` | 1.4 KB | GCP provider configuration |

### Configuration Files

| File | Purpose |
|------|---------|
| `terraform.tfvars.example` | Template with all options |
| `terraform.tfvars.dev` | Development (scale-to-zero, $10-20/month) |
| `terraform.tfvars.staging` | Staging (always-on, $50-100/month) |
| `terraform.tfvars.prod` | Production (HA, $3k-5k/month) |

### Documentation (4,200+ lines)

| File | Focus |
|------|-------|
| `README.md` | Complete reference documentation |
| `QUICKSTART.md` | 10-minute quick start |
| `DEPLOYMENT_GUIDE.md` | Step-by-step deployment |
| `COST_OPTIMIZATION.md` | Cost management strategies |
| `INFRASTRUCTURE_OVERVIEW.md` | Architecture and design |

### Tools

| File | Purpose |
|------|---------|
| `Makefile` | 40+ convenient commands |
| `.gitignore` | Safe git configuration |

## Features

### Cloud Run Services

- **3 independent services** with separate scaling
- **Resource allocation**: CPU (0.5-4 vCPU) and Memory (512Mi-4Gi) per service
- **Auto-scaling**: Configurable min/max instances per service
- **Health checks**: Startup and liveness probes
- **Request timeouts**: Per-service timeout configuration
- **Environment variables**: Service-specific and shared

### IAM & Security

- **Service accounts**: One per service with minimal required roles
- **Role-based access control**: Least privilege principle
- **Secret management**: Cloud Secret Manager integration
- **API-level access**: Services can only access what they need

### Networking

- **VPC isolation**: Optional custom VPC network
- **VPC Connector**: For private networking between services and GCP resources
- **Network policies**: Automatic ingress control
- **Service discovery**: Internal Cloud Run DNS

### Monitoring & Observability

- **Cloud Logging**: Structured logs for all services
- **Cloud Trace**: Distributed tracing support
- **Cloud Profiler**: Optional performance profiling
- **Custom metrics**: Application-defined metrics
- **Log queries**: Pre-built queries for all services

### Cost Management

- **Per-environment optimization**: Dev, staging, production
- **Scale-to-zero support**: Dev environment scales down when idle
- **Cost estimation**: Infracost integration
- **Budget alerts**: GCP budget configuration examples

## Architecture

### Service Topology

```
osiris-edge (Admissions)
    ↓
    └─→ invokes osiris-compiler (Deterministic Pipeline)
    ├─→ osiris-marketplace (Usage Reporting)
```

### Infrastructure

```
Cloud Run Services (3)
    ↓
VPC Connector (Optional)
    ↓
[Firestore | GCS | Service Control]
    ↓
Cloud Logging & Monitoring
```

## Requirements

### Software

- **Terraform** >= 1.0: https://www.terraform.io/downloads
- **Google Cloud SDK**: https://cloud.google.com/sdk/docs/install
- **Docker** (for building images): https://docs.docker.com/get-docker/
- **Git**: For version control

### GCP Setup

1. **Project**: Create a Google Cloud project
2. **Billing**: Enable billing for the project
3. **APIs**: Cloud Run, Compute Engine, Artifact Registry, Cloud Logging
4. **Credentials**: `gcloud auth application-default login`

## Deployment

### Development

```bash
terraform apply -var-file="terraform.tfvars.dev"
```

Features:
- Minimum resource allocation
- Scale to zero when idle
- Latest image tags
- ~$10-20/month cost

### Staging

```bash
terraform apply -var-file="terraform.tfvars.staging"
```

Features:
- Moderate resources
- Always-on (1 instance minimum)
- Version-specific image tags
- ~$50-100/month cost

### Production

```bash
terraform apply -var-file="terraform.tfvars.prod"
```

Features:
- High resources
- High availability (2+ instances minimum)
- Release version tags
- Load balancer and CDN
- ~$3k-5k/month cost

## Common Commands

### Deployment

```bash
# Initialize
terraform init

# Plan
terraform plan -var-file="terraform.tfvars.dev"

# Apply
terraform apply -var-file="terraform.tfvars.dev"

# Destroy
terraform destroy -var-file="terraform.tfvars.dev"
```

### Using Makefile

```bash
# Plan
make plan ENV=dev

# Deploy
make apply ENV=dev

# View logs
make logs ENV=dev

# Health check
make health ENV=dev

# Destroy
make destroy ENV=dev
```

## Configuration

All services are configured via Terraform variables with defaults:

```hcl
# Example: Deploy with custom settings
terraform apply -var-file="terraform.tfvars" \
  -var="edge_memory=2Gi" \
  -var="edge_max_instances=20" \
  -var="edge_image_tag=v0.2.0"
```

## Monitoring

### View Logs

```bash
# All services
make logs ENV=dev

# Specific service
make logs-edge ENV=dev

# Errors only
make logs-errors ENV=dev
```

### View Metrics

```bash
# Health checks
make health ENV=dev

# Cloud Console
gcloud monitoring dashboards list
```

### Performance

```bash
# Enable profiler in production
terraform apply -var-file="terraform.tfvars.prod" \
  -var="enable_profiler=true"
```

## Cost Management

### Development Cost Optimization

```hcl
# Min instances = 0 for scale-to-zero
# Lower CPU and memory allocation
# Disable VPC connector and load balancer
# Expected cost: $10-20/month
```

### Production Cost Control

```hcl
# Set reasonable max_instances limits
# Use load balancer only if needed
# Enable CDN for cacheable content
# Monitor and alert on budget
```

See [terraform/COST_OPTIMIZATION.md](./terraform/COST_OPTIMIZATION.md) for details.

## CI/CD Integration

### GitHub Actions

```yaml
name: Deploy A2A-RS

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: hashicorp/setup-terraform@v2
      - name: Terraform Apply
        run: |
          cd terraform
          terraform init
          terraform apply -auto-approve -var-file="terraform.tfvars.prod"
```

## Security Best Practices

1. **Least Privilege**: Each service has minimal required IAM roles
2. **Secret Management**: Use Cloud Secret Manager for sensitive data
3. **Network Isolation**: VPC Connector for private networking
4. **Audit Logging**: Cloud Audit Logs for compliance
5. **Service Accounts**: Dedicated per-service accounts

## Troubleshooting

### Service won't start

```bash
# Check logs
make logs ENV=dev

# Verify image exists
gcloud artifacts docker images list \
  us-central1-docker.pkg.dev/PROJECT_ID/a2a-rs
```

### High costs

```bash
# Review resource allocation
make output ENV=prod

# Reduce max instances
terraform apply -var-file="terraform.tfvars.prod" \
  -var="edge_max_instances=30"
```

### Terraform errors

```bash
# Validate
terraform validate

# Format
terraform fmt -recursive

# Refresh state
terraform refresh -var-file="terraform.tfvars.dev"
```

## Documentation Guide

Start here based on your need:

| Need | Read |
|------|------|
| Quick 10-min setup | [QUICKSTART.md](./terraform/QUICKSTART.md) |
| Step-by-step guide | [DEPLOYMENT_GUIDE.md](./terraform/DEPLOYMENT_GUIDE.md) |
| Full reference | [README.md](./terraform/README.md) |
| Cost management | [COST_OPTIMIZATION.md](./terraform/COST_OPTIMIZATION.md) |
| Architecture details | [INFRASTRUCTURE_OVERVIEW.md](./terraform/INFRASTRUCTURE_OVERVIEW.md) |

## File Structure

```
terraform/
├── main.tf                      # Core infrastructure
├── variables.tf                 # Input variables
├── outputs.tf                   # Output values
├── provider.tf                  # Provider config
├── terraform.tfvars.*           # Environment configs
├── Makefile                     # Convenient commands
├── .gitignore                   # Git ignore rules
├── QUICKSTART.md                # Quick start guide
├── README.md                    # Full documentation
├── DEPLOYMENT_GUIDE.md          # Deployment walkthrough
├── COST_OPTIMIZATION.md         # Cost management
└── INFRASTRUCTURE_OVERVIEW.md   # Architecture guide
```

## What Gets Created

### GCP Resources

| Type | Count | Purpose |
|------|-------|---------|
| Cloud Run Services | 3 | Edge, Compiler, Marketplace |
| Service Accounts | 3 | One per service |
| IAM Bindings | 15+ | Permissions and access |
| VPC Network | 1 | Custom network (optional) |
| VPC Connector | 1 | Private networking (optional) |
| APIs | 10+ | Required Google APIs |

### Total Lines of Code

- **Terraform**: 800+ lines
- **Documentation**: 3,400+ lines
- **Makefile**: 250+ lines
- **Total**: 4,200+ lines

## Next Steps

1. **Read**: [terraform/QUICKSTART.md](./terraform/QUICKSTART.md) (5 minutes)
2. **Deploy**: [terraform/DEPLOYMENT_GUIDE.md](./terraform/DEPLOYMENT_GUIDE.md) (15 minutes)
3. **Reference**: [terraform/README.md](./terraform/README.md) (comprehensive guide)
4. **Optimize**: [terraform/COST_OPTIMIZATION.md](./terraform/COST_OPTIMIZATION.md) (cost management)

## Support

- **Questions?** See [terraform/README.md](./terraform/README.md#troubleshooting)
- **Deployment issues?** See [terraform/DEPLOYMENT_GUIDE.md](./terraform/DEPLOYMENT_GUIDE.md#troubleshooting)
- **Cost questions?** See [terraform/COST_OPTIMIZATION.md](./terraform/COST_OPTIMIZATION.md)
- **Architecture?** See [terraform/INFRASTRUCTURE_OVERVIEW.md](./terraform/INFRASTRUCTURE_OVERVIEW.md)

## Related Files

- [CLAUDE.md](./CLAUDE.md) - Project conventions and setup
- [README.md](./README.md) - Project overview
- Individual service docs: See service directories

---

**Start deploying now**: `cd terraform && make help`

Last updated: February 10, 2026
