# A2A-RS Terraform Configuration

Infrastructure-as-Code configuration for deploying A2A-RS services to Google Cloud Run.

## Overview

This Terraform configuration manages the deployment of three Cloud Run services:

1. **osiris-edge**: HTTP/WebSocket server for admissions control, WIP gate, and refusal engine
2. **osiris-compiler**: Deterministic compilation with 7-stage pipeline
3. **osiris-marketplace**: Google Workspace integrations and Cloud Service Control usage reporting

## Architecture

### Services

```
┌─────────────────────────────────────────────────────────────────┐
│                     Cloud Run Services                           │
├──────────────────────┬──────────────────────┬──────────────────┤
│   osiris-edge        │  osiris-compiler     │ osiris-marketplace│
│                      │                      │                   │
│ - Admissions control │ - Type checker (Σ)   │ - Workspace APIs  │
│ - WIP gate           │ - Guards (H)         │ - Service Control │
│ - Refusal engine     │ - Orderer (Λ)        │ - Usage reporting │
│ - Normalizer         │ - Kernel (K)         │                   │
│                      │ - Invariants (Q)     │                   │
│                      │ - Writer             │                   │
│                      │ - Receipt Builder    │                   │
└──────────────────────┴──────────────────────┴──────────────────┘
         ↓                    ↓                      ↓
    Service Account     Service Account       Service Account
    (roles/...*)        (roles/...*)          (roles/...*)
         ↓                    ↓                      ↓
      Firestore           Firestore            Firestore/GCS
      GCS/Secrets         GCS/Secrets          Service Control
```

### Networking

```
┌────────────────────────────────────────────────────────┐
│              Custom VPC Network (optional)             │
├────────────────────────────────────────────────────────┤
│  Subnet: 10.0.0.0/24                                   │
│  ┌──────────────────────────────────────────────────┐  │
│  │   VPC Access Connector (10.8.0.0/28)            │  │
│  │   - Min instances: 2                             │  │
│  │   - Max instances: 10                            │  │
│  │   - Provides private networking for services    │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

### IAM

Each service has a dedicated service account with specific IAM roles:

- **Common roles**:
  - `roles/artifactregistry.reader` - Pull container images
  - `roles/logging.logWriter` - Write logs
  - `roles/monitoring.metricWriter` - Write metrics

- **Service-specific roles**:
  - Edge: `roles/run.invoker` for compiler invocation
  - Compiler: None additional
  - Marketplace: `roles/servicemanagement.admin` for Service Control

## Files

| File | Purpose |
|------|---------|
| `main.tf` | Cloud Run services, service accounts, IAM, VPC connector |
| `variables.tf` | Input variables with validation |
| `outputs.tf` | Output values (endpoints, accounts, etc.) |
| `provider.tf` | Provider configuration |
| `terraform.tfvars.example` | Example configuration template |
| `terraform.tfvars.dev` | Development environment config |
| `terraform.tfvars.staging` | Staging environment config |
| `terraform.tfvars.prod` | Production environment config |
| `README.md` | This file |

## Prerequisites

1. **Google Cloud Project**
   - Create a project: `gcloud projects create a2a-rs-<env>`
   - Enable billing
   - Set default project: `gcloud config set project a2a-rs-<env>`

2. **Terraform**
   - Install Terraform >= 1.0: https://www.terraform.io/downloads

3. **Google Cloud CLI**
   - Install gcloud: https://cloud.google.com/sdk/docs/install
   - Authenticate: `gcloud auth application-default login`

4. **Container Images**
   - Build and push images to Artifact Registry (see below)

## Setup

### 1. Initialize Terraform

```bash
cd terraform
terraform init
```

### 2. Create Artifact Registry

```bash
# Enable Artifact Registry API
gcloud services enable artifactregistry.googleapis.com

# Create repository
gcloud artifacts repositories create a2a-rs \
  --repository-format=docker \
  --location=us-central1 \
  --description="A2A-RS container images"
```

### 3. Build and Push Container Images

From workspace root, build images:

```bash
# Build edge service
docker build -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest \
  -f osiris-edge/Dockerfile .
gcloud docker -- push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest

# Build compiler service
docker build -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-compiler:latest \
  -f osiris-compiler/Dockerfile .
gcloud docker -- push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-compiler:latest

# Build marketplace service
docker build -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-marketplace:latest \
  -f osiris-marketplace/Dockerfile .
gcloud docker -- push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-marketplace:latest
```

Or use Cloud Build:

```bash
gcloud builds submit --config=cloudbuild.yaml
```

### 4. Create Configuration

Copy and customize the environment configuration:

```bash
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your project ID and settings
```

Or use pre-built environment configs:

```bash
# Development
terraform apply -var-file="terraform.tfvars.dev"

# Staging
terraform apply -var-file="terraform.tfvars.staging"

# Production
terraform apply -var-file="terraform.tfvars.prod"
```

### 5. Create Remote State (Recommended)

For production, store Terraform state in GCS:

```bash
# Create GCS bucket for state
gsutil mb gs://a2a-rs-terraform-state

# Enable versioning
gsutil versioning set on gs://a2a-rs-terraform-state

# Uncomment backend in provider.tf
```

### 6. Review and Apply

```bash
# Validate configuration
terraform validate

# Format configuration
terraform fmt

# Review planned changes
terraform plan -var-file="terraform.tfvars.dev"

# Apply configuration
terraform apply -var-file="terraform.tfvars.dev"
```

## Deployment

### Complete Deployment

```bash
# 1. Initialize
terraform init

# 2. Plan
terraform plan -var-file="terraform.tfvars.dev"

# 3. Apply
terraform apply -var-file="terraform.tfvars.dev"

# 4. Verify
terraform output services_summary
```

### Access Services

After deployment, services are accessible at their HTTPS endpoints:

```bash
# Get service URLs
terraform output -json services_summary | jq '.*.url'

# Test with curl (requires authentication)
curl -H "Authorization: Bearer $(gcloud auth print-identity-token)" \
  https://osiris-edge-xxxxx-uc.a.run.app/health
```

### Grant Access

Grant users permission to invoke services:

```bash
# Allow user to invoke edge service
gcloud run services add-iam-policy-binding osiris-edge \
  --member=user:user@example.com \
  --role=roles/run.invoker \
  --region=us-central1

# Allow service account to invoke service
gcloud run services add-iam-policy-binding osiris-compiler \
  --member=serviceAccount:sa@project.iam.gserviceaccount.com \
  --role=roles/run.invoker \
  --region=us-central1
```

## Configuration Variables

### Project & Region

| Variable | Description | Default |
|----------|-------------|---------|
| `project_id` | GCP Project ID | Required |
| `region` | GCP region | `us-central1` |
| `environment` | Environment (dev/staging/prod) | `dev` |

### Service Resources

Each service (edge, compiler, marketplace) has configurable:

- `{service}_memory`: Memory allocation (512Mi, 1Gi, 2Gi, 4Gi)
- `{service}_cpu`: CPU allocation (0.5, 1, 2, 4)
- `{service}_timeout`: Request timeout (1-3600 seconds)
- `{service}_concurrency`: Max concurrent requests per instance
- `{service}_min_instances`: Minimum instances (0+)
- `{service}_max_instances`: Maximum instances

Example:

```hcl
edge_memory        = "1Gi"
edge_cpu           = "1"
edge_min_instances = 1
edge_max_instances = 10
```

### Networking

| Variable | Description | Default |
|----------|-------------|---------|
| `enable_vpc_connector` | Enable private VPC networking | `false` |
| `create_custom_network` | Create custom VPC | `false` |
| `network_name` | VPC network name | `a2a-rs-network` |
| `subnet_cidr` | Subnet CIDR block | `10.0.0.0/24` |
| `vpc_connector_cidr` | VPC connector CIDR | `10.8.0.0/28` |

### Features

| Variable | Description | Default |
|----------|-------------|---------|
| `enable_public_ingress` | Allow public access | `false` |
| `enable_firestore` | Enable Firestore access | `true` |
| `enable_gcs` | Enable Cloud Storage access | `true` |
| `enable_service_control` | Enable Service Control API | `true` |
| `enable_load_balancer` | Enable external load balancer | `false` |
| `enable_cdn` | Enable CDN | `false` |

### Monitoring

| Variable | Description | Default |
|----------|-------------|---------|
| `enable_tracing` | Enable Cloud Trace | `true` |
| `enable_profiler` | Enable Cloud Profiler | `false` |
| `log_retention_days` | Log retention in days | `30` |

## Monitoring & Logging

### View Logs

```bash
# All a2a-rs services
gcloud logs read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name=~"(osiris-edge|osiris-compiler|osiris-marketplace)"' \
  --limit 50 \
  --format=json

# Specific service
gcloud logs read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge"' \
  --limit 50

# With filtering
gcloud logs read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge" AND severity=ERROR' \
  --limit 10
```

### Cloud Logging Console

Open Cloud Logging in Cloud Console:

```
https://console.cloud.google.com/logs/query?project=YOUR_PROJECT_ID
```

### Metrics

View custom metrics in Cloud Monitoring:

```
https://console.cloud.google.com/monitoring?project=YOUR_PROJECT_ID
```

Custom metrics namespace: `custom.googleapis.com/a2a-rs`

### Health Checks

Test service health:

```bash
# Get identity token
TOKEN=$(gcloud auth print-identity-token)

# Health check each service
curl -H "Authorization: Bearer $TOKEN" \
  $(terraform output -json | jq -r .edge_service_url)/health

curl -H "Authorization: Bearer $TOKEN" \
  $(terraform output -json | jq -r .compiler_service_url)/health

curl -H "Authorization: Bearer $TOKEN" \
  $(terraform output -json | jq -r .marketplace_service_url)/health
```

## Environment-Specific Deployments

### Development

```bash
terraform apply -var-file="terraform.tfvars.dev"
```

Features:
- Lower resource allocation
- Auto-scaling: 0-2 instances
- Latest image tags
- Internal-only access
- Profiler enabled

### Staging

```bash
terraform apply -var-file="terraform.tfvars.staging"
```

Features:
- Medium resource allocation
- Auto-scaling: 1-5/10 instances
- Version-specific image tags (v0.1.0-staging)
- Public access (IAM-controlled)
- Standard monitoring

### Production

```bash
terraform apply -var-file="terraform.tfvars.prod"
```

Features:
- High resource allocation
- Always-on (minimum 2 instances)
- Auto-scaling: 2-30/50/100 instances
- Release version tags (v0.1.0)
- Public access with load balancer
- CDN enabled
- Extended log retention (90 days)

## Updating Services

### Update Service Configuration

Modify `terraform.tfvars` and apply:

```bash
terraform apply -var-file="terraform.tfvars"
```

### Update Container Image

Update image tag and apply:

```bash
# New image tag
terraform apply -var-file="terraform.tfvars" \
  -var="edge_image_tag=v0.2.0"
```

### Scale Services

Update min/max instances:

```bash
terraform apply -var-file="terraform.tfvars" \
  -var="edge_max_instances=20" \
  -var="compiler_max_instances=50"
```

## Cleanup

### Destroy All Resources

```bash
# Review what will be deleted
terraform plan -destroy -var-file="terraform.tfvars"

# Delete all resources
terraform destroy -var-file="terraform.tfvars"
```

### Selective Cleanup

```bash
# Delete only edge service
terraform destroy -var-file="terraform.tfvars" \
  -target=google_cloud_run_v2_service.edge

# Delete service accounts only
terraform destroy -var-file="terraform.tfvars" \
  -target=google_service_account.edge \
  -target=google_service_account.compiler \
  -target=google_service_account.marketplace
```

## Troubleshooting

### Service Fails to Start

1. Check logs:
   ```bash
   gcloud logs read --filter='resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge"' --limit 50
   ```

2. Verify service account IAM roles:
   ```bash
   gcloud projects get-iam-policy PROJECT_ID --flatten="bindings[].members" --filter="bindings.members:serviceAccount:*"
   ```

3. Check image exists in Artifact Registry:
   ```bash
   gcloud artifacts docker images list us-central1-docker.pkg.dev/PROJECT_ID/a2a-rs
   ```

### VPC Connector Issues

```bash
# List VPC connectors
gcloud compute networks vpc-access connectors list --region=us-central1

# Check connector status
gcloud compute networks vpc-access connectors describe a2a-rs-network-connector --region=us-central1
```

### Terraform State Issues

```bash
# Validate state
terraform validate

# Refresh state
terraform refresh -var-file="terraform.tfvars"

# Import existing resource
terraform import google_cloud_run_v2_service.edge projects/PROJECT_ID/locations/us-central1/services/osiris-edge
```

## Cost Optimization

### Development

- Set `edge_min_instances = 0` for auto-scale down
- Use lower CPU/memory for dev services
- Disable load balancer
- Disable CDN

### Production

- Use reserved capacity for consistent minimum load
- Enable CDN for static content
- Use custom domain with SSL certificate
- Monitor and adjust max instances

## Security

### Network Security

- Enable VPC connector for private networking
- Use `enable_vpc_connector=true` with `create_custom_network=true`
- Services communicate privately without internet exposure

### Identity & Access

- Each service has dedicated service account
- Minimal IAM roles (least privilege)
- Enable Cloud Audit Logs for all operations

### Secrets Management

Define secrets in Cloud Secret Manager and reference in config:

```hcl
edge_secrets = {
  "DATABASE_URL" = {
    secret  = "edge-database-url"
    version = "latest"
  }
}
```

Create secret:

```bash
echo -n "postgresql://..." | gcloud secrets create edge-database-url --data-file=-
```

## Maintenance

### Regular Tasks

1. **Weekly**: Review logs for errors
2. **Monthly**: Check metrics and cost
3. **Quarterly**: Update base images and dependencies
4. **Annually**: Review IAM policies and security

### Backup & Disaster Recovery

- Terraform state stored in GCS with versioning
- Container images backed up in Artifact Registry
- Configuration tracked in Git

### Updates

```bash
# Update Terraform providers
terraform init -upgrade

# Update to latest Google provider
terraform {
  required_providers {
    google = {
      version = "~> 5.10"
    }
  }
}
```

## CI/CD Integration

### GitHub Actions

Example workflow:

```yaml
name: Deploy A2A-RS Services

on:
  push:
    branches: [main]
    paths:
      - 'terraform/**'

jobs:
  terraform:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: hashicorp/setup-terraform@v2
      - name: Terraform Init
        run: terraform -chdir=terraform init
      - name: Terraform Plan
        run: terraform -chdir=terraform plan -var-file="terraform.tfvars.prod"
      - name: Terraform Apply
        if: github.ref == 'refs/heads/main'
        run: terraform -chdir=terraform apply -auto-approve -var-file="terraform.tfvars.prod"
```

## References

- [Cloud Run Documentation](https://cloud.google.com/run/docs)
- [Terraform Google Provider](https://registry.terraform.io/providers/hashicorp/google/latest/docs)
- [Cloud Run Security Best Practices](https://cloud.google.com/run/docs/securing/managing-access)
- [VPC Access Connector](https://cloud.google.com/vpc/docs/configure-private-service-connection)

## Support

For issues or questions:

1. Check logs: `gcloud logs read --filter='resource.type="cloud_run_revision"'`
2. Review Terraform output: `terraform show`
3. Check GCP Console: https://console.cloud.google.com/
4. Review CLAUDE.md for project conventions

## License

Part of the A2A-RS project. See root LICENSE file.
