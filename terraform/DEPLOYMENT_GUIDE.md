# A2A-RS Deployment Guide

Complete step-by-step guide for deploying the A2A-RS services to Google Cloud Run.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start (5 minutes)](#quick-start-5-minutes)
3. [Detailed Setup](#detailed-setup)
4. [Deployment](#deployment)
5. [Verification](#verification)
6. [Next Steps](#next-steps)

## Prerequisites

### Required Tools

Install these tools before starting:

1. **Google Cloud SDK** (gcloud)
   ```bash
   # Installation: https://cloud.google.com/sdk/docs/install
   gcloud --version  # Verify installation
   ```

2. **Terraform** >= 1.0
   ```bash
   # Installation: https://www.terraform.io/downloads
   terraform --version  # Verify installation
   ```

3. **Docker**
   ```bash
   # Installation: https://docs.docker.com/get-docker/
   docker --version  # Verify installation
   ```

4. **Git**
   ```bash
   git --version  # Verify installation
   ```

### GCP Account Setup

1. Create a Google Cloud project:
   ```bash
   gcloud projects create a2a-rs-dev --name="A2A-RS Development"
   gcloud config set project a2a-rs-dev
   ```

2. Enable billing:
   ```bash
   # Open Cloud Console and enable billing for the project
   # https://console.cloud.google.com/billing
   ```

3. Authenticate:
   ```bash
   gcloud auth application-default login
   gcloud auth configure-docker us-central1-docker.pkg.dev
   ```

4. Set default region:
   ```bash
   gcloud config set compute/region us-central1
   ```

## Quick Start (5 minutes)

For a quick development deployment:

```bash
# 1. Clone repository
cd /home/user/a2a-rs

# 2. Initialize Terraform
cd terraform
terraform init

# 3. Copy dev config
cp terraform.tfvars.example terraform.tfvars
# Edit with your project ID:
#   project_id = "your-project-id"

# 4. Build and push images
# (Assumes Dockerfiles exist in each service directory)
make build-images  # Build locally
make push-images   # Push to Artifact Registry

# 5. Deploy
make apply ENV=dev

# 6. Verify
make output ENV=dev
```

## Detailed Setup

### Step 1: Create GCP Project

```bash
# Create project for development
gcloud projects create a2a-rs-dev \
  --name="A2A-RS Development" \
  --set-as-default

# Verify
gcloud config list
```

### Step 2: Enable Required APIs

```bash
# Enable APIs needed for deployment
gcloud services enable \
  run.googleapis.com \
  compute.googleapis.com \
  artifactregistry.googleapis.com \
  logging.googleapis.com \
  monitoring.googleapis.com \
  cloudresourcemanager.googleapis.com \
  iam.googleapis.com \
  firestore.googleapis.com \
  storage.googleapis.com \
  servicemanagement.googleapis.com
```

### Step 3: Create Artifact Registry

```bash
# Create Docker repository
gcloud artifacts repositories create a2a-rs \
  --repository-format=docker \
  --location=us-central1 \
  --description="A2A-RS container images"

# Verify
gcloud artifacts repositories list
```

### Step 4: Build Container Images

From the workspace root, build images for each service:

#### Option A: Local Docker Build

```bash
# Build edge service
docker build \
  -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest \
  -f osiris-edge/Dockerfile .

# Build compiler service
docker build \
  -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-compiler:latest \
  -f osiris-compiler/Dockerfile .

# Build marketplace service
docker build \
  -t us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-marketplace:latest \
  -f osiris-marketplace/Dockerfile .
```

Note: If Dockerfiles don't exist yet, see [Creating Dockerfiles](#creating-dockerfiles) below.

#### Option B: Cloud Build

```bash
# Using Cloud Build (recommended for CI/CD)
gcloud builds submit \
  --tag us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest
```

### Step 5: Push Images to Artifact Registry

```bash
# Push to Artifact Registry
docker push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest
docker push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-compiler:latest
docker push us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-marketplace:latest

# Verify
gcloud artifacts docker images list us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs
```

### Step 6: Initialize Terraform

```bash
cd /home/user/a2a-rs/terraform

# Initialize working directory
terraform init

# Verify
terraform version
terraform validate
```

### Step 7: Create Terraform Configuration

```bash
# Copy example configuration
cp terraform.tfvars.example terraform.tfvars

# Edit with your project settings
# Update:
#   project_id = "a2a-rs-dev"
#   region = "us-central1"
#   environment = "dev"

# Or use pre-configured environment file
cp terraform.tfvars.dev terraform.tfvars
```

## Deployment

### Development Deployment

```bash
cd /home/user/a2a-rs/terraform

# Plan changes
terraform plan -var-file="terraform.tfvars.dev"

# Review the plan carefully
# Verify:
#   - Correct project ID
#   - Correct image tags
#   - Service account permissions
#   - Network configuration

# Apply
terraform apply -var-file="terraform.tfvars.dev"

# Confirm by typing "yes" when prompted
```

### Staging Deployment

```bash
# Ensure images are built and pushed for staging
docker tag \
  us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:latest \
  us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:v0.1.0-staging

docker push \
  us-central1-docker.pkg.dev/a2a-rs-dev/a2a-rs/osiris-edge:v0.1.0-staging

# Update terraform.tfvars.staging with new image tags

# Deploy
terraform apply -var-file="terraform.tfvars.staging"
```

### Production Deployment

```bash
# Create production GCP project
gcloud projects create a2a-rs-prod --name="A2A-RS Production"
gcloud config set project a2a-rs-prod

# Repeat setup steps 2-5 for production project

# Deploy with high availability
terraform apply -var-file="terraform.tfvars.prod"
```

## Verification

### Check Deployment Status

```bash
# List deployed services
gcloud run services list --region=us-central1

# Get service details
gcloud run services describe osiris-edge --region=us-central1

# View revisions
gcloud run revisions list --service=osiris-edge --region=us-central1
```

### Test Service Endpoints

```bash
# Get service URLs
terraform output -json | jq '.*.url'

# Get identity token for testing
TOKEN=$(gcloud auth print-identity-token)

# Test edge service
curl -H "Authorization: Bearer $TOKEN" \
  https://osiris-edge-xxxxx-uc.a.run.app/health

# Test compiler service
curl -H "Authorization: Bearer $TOKEN" \
  https://osiris-compiler-xxxxx-uc.a.run.app/health

# Test marketplace service
curl -H "Authorization: Bearer $TOKEN" \
  https://osiris-marketplace-xxxxx-uc.a.run.app/health
```

### Check Logs

```bash
# View all service logs
make logs ENV=dev

# View specific service logs
make logs-edge ENV=dev

# View errors only
make logs-errors ENV=dev

# View raw Cloud Logging
gcloud logs read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge"' \
  --limit 50 \
  --format=json | jq
```

### Monitor Metrics

```bash
# Open Cloud Monitoring console
gcloud monitoring dashboards list

# View service metrics
gcloud monitoring time-series list \
  --filter='metric.type="run.googleapis.com/request_count"'
```

## Creating Dockerfiles

If Dockerfiles don't exist in each service directory, create them:

### osiris-edge/Dockerfile

```dockerfile
FROM rust:1.85-slim as builder

WORKDIR /workspace

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY osiris-edge ./osiris-edge
COPY a2a-rs ./a2a-rs

# Build
RUN cargo build --release -p osiris-edge

# Runtime image
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /workspace/target/release/osiris-edge /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["osiris-edge"]
```

### osiris-compiler/Dockerfile

```dockerfile
FROM rust:1.85-slim as builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY osiris-compiler ./osiris-compiler
COPY a2a-rs ./a2a-rs

RUN cargo build --release -p osiris-compiler

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/osiris-compiler /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["osiris-compiler"]
```

### osiris-marketplace/Dockerfile

```dockerfile
FROM rust:1.85-slim as builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY osiris-marketplace ./osiris-marketplace
COPY a2a-rs ./a2a-rs

RUN cargo build --release -p osiris-marketplace

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/osiris-marketplace /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["osiris-marketplace"]
```

## Next Steps

### 1. Grant Access to Users

```bash
# Allow user to invoke edge service
gcloud run services add-iam-policy-binding osiris-edge \
  --member=user:user@example.com \
  --role=roles/run.invoker \
  --region=us-central1

# Allow all authenticated users
gcloud run services add-iam-policy-binding osiris-edge \
  --member=allAuthenticatedUsers \
  --role=roles/run.invoker \
  --region=us-central1
```

### 2. Set Up Monitoring

```bash
# Create alerting policy for errors
gcloud alpha monitoring policies create \
  --notification-channels=CHANNEL_ID \
  --display-name="A2A-RS Service Errors" \
  --condition-display-name="High error rate" \
  --condition-threshold-value=0.05
```

### 3. Configure Custom Domain

```bash
# Map custom domain to Cloud Run service
gcloud run domain-mappings create \
  --service=osiris-edge \
  --domain=edge.example.com \
  --region=us-central1
```

### 4. Set Up CI/CD Pipeline

Create `.github/workflows/deploy.yml`:

```yaml
name: Deploy to Cloud Run

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Set up Cloud SDK
        uses: google-github-actions/setup-gcloud@v1
        with:
          project_id: ${{ secrets.GCP_PROJECT_ID }}
          service_account_key: ${{ secrets.GCP_SA_KEY }}
          export_default_credentials: true

      - name: Build and push images
        run: |
          gcloud builds submit --tag us-central1-docker.pkg.dev/$PROJECT_ID/a2a-rs/osiris-edge

      - name: Deploy
        run: |
          cd terraform
          terraform init
          terraform apply -auto-approve -var-file="terraform.tfvars.prod"
```

### 5. Set Up Cost Monitoring

```bash
# Install Infracost
curl https://raw.githubusercontent.com/infracost/infracost/master/scripts/install.sh | sh

# Estimate costs
make costs ENV=prod
```

## Troubleshooting

### Deployment Fails

```bash
# Check Terraform validation
terraform validate

# Format issues
terraform fmt -recursive

# Check state
terraform state list

# Plan with verbose output
terraform plan -var-file="terraform.tfvars" -var="log_level=debug"
```

### Service Won't Start

```bash
# Check logs
gcloud logs read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge"' \
  --limit 50

# Verify image exists
gcloud artifacts docker images list us-central1-docker.pkg.dev/PROJECT_ID/a2a-rs

# Check service account permissions
gcloud projects get-iam-policy PROJECT_ID \
  --flatten="bindings[].members" \
  --filter="bindings.members:serviceAccount:*"
```

### IAM Errors

```bash
# Check current identity
gcloud auth list

# List service accounts
gcloud iam service-accounts list

# Check role assignments
gcloud projects get-iam-policy PROJECT_ID \
  --flatten="bindings[].members" \
  --format="table(bindings.role)"
```

### Network Connectivity Issues

```bash
# Verify VPC connector is running
gcloud compute networks vpc-access connectors list --region=us-central1

# Check connector status
gcloud compute networks vpc-access connectors describe a2a-rs-network-connector \
  --region=us-central1
```

## Cleanup

### Destroy Development Environment

```bash
# Plan destruction
terraform plan -destroy -var-file="terraform.tfvars.dev"

# Confirm and destroy
terraform destroy -var-file="terraform.tfvars.dev"
```

### Delete GCP Project

```bash
# List projects
gcloud projects list | grep a2a-rs

# Delete project
gcloud projects delete a2a-rs-dev
```

## Rollback

To rollback to a previous version:

```bash
# List previous versions
gcloud run revisions list --service=osiris-edge --region=us-central1

# Deploy previous revision
gcloud run services update-traffic osiris-edge \
  --to-revisions=osiris-edge-00010=100 \
  --region=us-central1
```

Or with Terraform:

```bash
# Update image tag to previous version
terraform apply -var-file="terraform.tfvars" \
  -var="edge_image_tag=v0.0.9"
```

## Support

- Check logs: `make logs ENV=dev`
- Review configuration: `make show ENV=dev`
- List resources: `make state-list`
- View documentation: See [README.md](./README.md)

## Additional Resources

- [Cloud Run Documentation](https://cloud.google.com/run/docs)
- [Terraform Google Provider](https://registry.terraform.io/providers/hashicorp/google/latest)
- [Cloud Run Security](https://cloud.google.com/run/docs/securing/managing-access)
- [VPC Access Connector](https://cloud.google.com/vpc/docs/configure-private-service-connection)
- [A2A-RS Documentation](../README.md)
