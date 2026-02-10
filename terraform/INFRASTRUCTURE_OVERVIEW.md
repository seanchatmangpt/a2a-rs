# A2A-RS Infrastructure Overview

Complete guide to the Terraform-managed infrastructure for A2A-RS workspace.

## What Was Created

A production-ready Terraform configuration for deploying three microservices to Google Cloud Run:

1. **osiris-edge** - Admissions control, WIP gate, refusal engine
2. **osiris-compiler** - Deterministic compilation with 7-stage pipeline
3. **osiris-marketplace** - Google Workspace integrations, usage reporting

## Architecture

### Services Deployment

```
┌──────────────────────────────────────────────────────────────┐
│                    Google Cloud Platform                      │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │            Artifact Registry (Container Images)         │ │
│  └────────────────────────┬────────────────────────────────┘ │
│                           │                                   │
│         ┌─────────────────┼─────────────────┐               │
│         │                 │                 │               │
│         ▼                 ▼                 ▼               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ osiris-edge │  │  compiler   │  │ marketplace │         │
│  │  (Cloud Run)│  │  (Cloud Run) │  │ (Cloud Run) │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│         └────────────────┼────────────────┘                 │
│                          │                                  │
│  ┌──────────────────────┴──────────────────────┐            │
│  │  Internal Networking (VPC Connector)        │            │
│  ├──────────────────────────────────────────────┤            │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │            │
│  │  │Firestore │  │    GCS   │  │ Service  │  │            │
│  │  │          │  │          │  │ Control  │  │            │
│  │  └──────────┘  └──────────┘  └──────────┘  │            │
│  └─────────────────────────────────────────────┘            │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           Cloud Logging & Monitoring                    │ │
│  │  - Structured logs for all services                     │ │
│  │  - Custom metrics and traces                            │ │
│  │  - Cloud Profiler for performance analysis              │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                                │
└──────────────────────────────────────────────────────────────┘
```

### Networking

```
┌────────────────────────────────────────────┐
│         Custom VPC Network                  │
│  (a2a-rs-network, 10.0.0.0/24)             │
├────────────────────────────────────────────┤
│                                             │
│  ┌──────────────────────────────────────┐  │
│  │   VPC Access Connector               │  │
│  │  (10.8.0.0/28)                       │  │
│  │  Enables private networking between  │  │
│  │  Cloud Run services and internal     │  │
│  │  GCP resources                       │  │
│  └──────────────────────────────────────┘  │
│                                             │
│  ┌──────────────────────────────────────┐  │
│  │      Subnet: 10.0.0.0/24             │  │
│  │  Used for additional compute         │  │
│  │  resources if needed                 │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘
```

### IAM & Security

```
┌──────────────────────────────────────────────────────────┐
│                 Service Accounts                          │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  osiris-edge Service Account                       │ │
│  │  - artifactregistry.reader (pull images)           │ │
│  │  - logging.logWriter                               │ │
│  │  - monitoring.metricWriter                         │ │
│  │  - run.invoker (can invoke compiler)               │ │
│  │  - datastore.user (Firestore)                      │ │
│  │  - storage.objectAdmin (GCS)                       │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  osiris-compiler Service Account                   │ │
│  │  - artifactregistry.reader (pull images)           │ │
│  │  - logging.logWriter                               │ │
│  │  - monitoring.metricWriter                         │ │
│  │  - datastore.user (Firestore)                      │ │
│  │  - storage.objectAdmin (GCS)                       │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  osiris-marketplace Service Account                │ │
│  │  - artifactregistry.reader (pull images)           │ │
│  │  - logging.logWriter                               │ │
│  │  - monitoring.metricWriter                         │ │
│  │  - datastore.user (Firestore)                      │ │
│  │  - storage.objectAdmin (GCS)                       │ │
│  │  - servicemanagement.admin (Service Control)       │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

## Directory Structure

```
terraform/
├── main.tf                      # Main infrastructure (services, IAM, VPC)
├── variables.tf                 # Input variable definitions
├── outputs.tf                   # Output values
├── provider.tf                  # GCP provider configuration
├── terraform.tfvars.example     # Example configuration template
├── terraform.tfvars.dev         # Development environment config
├── terraform.tfvars.staging     # Staging environment config
├── terraform.tfvars.prod        # Production environment config
├── .gitignore                   # Git ignore rules
├── Makefile                     # Convenient commands
├── QUICKSTART.md                # 10-minute quick start (START HERE!)
├── README.md                    # Full documentation
├── DEPLOYMENT_GUIDE.md          # Step-by-step deployment instructions
├── COST_OPTIMIZATION.md         # Cost management strategies
└── INFRASTRUCTURE_OVERVIEW.md   # This file
```

## File Descriptions

### Configuration Files

#### main.tf (21 KB)
**The core infrastructure file**

Contains:
- Local variables for service configurations
- Cloud Run service definitions (edge, compiler, marketplace)
- Service account creation (3 accounts)
- IAM role assignments for each service
- VPC networking and connector setup
- Health check configurations
- Startup and liveness probes
- Scaling policies
- API enablement

Key resources:
- `google_cloud_run_v2_service` (3×) - The Cloud Run services
- `google_service_account` (3×) - Service accounts
- `google_project_iam_member` (15+) - IAM role assignments
- `google_vpc_access_connector` - Private networking
- `google_compute_network` - Custom VPC
- `google_compute_subnetwork` - Subnet

#### variables.tf (14 KB)
**Input variable definitions with validation**

Organized sections:
- Project & Region - GCP settings
- Artifact Registry - Container image configuration
- Service-specific variables (edge, compiler, marketplace):
  - Memory allocation (512Mi to 4Gi)
  - CPU allocation (0.5 to 4)
  - Timeout configuration
  - Concurrency limits
  - Auto-scaling (min/max instances)
  - Environment variables
  - Secrets references
- Common configuration - Shared across services
- Networking - VPC and connector settings
- Feature flags - Firestore, GCS, Service Control, etc.
- Monitoring - Tracing, profiler, log retention
- Tags & Labels - Resource identification

All variables include:
- Type definition
- Default values
- Validation rules with error messages
- Detailed descriptions

#### outputs.tf (11 KB)
**Output values for accessing deployed resources**

Sections:
- Service endpoints - HTTPS URLs for each service
- Service identifiers - Names and revisions
- Service accounts - Email and IDs
- VPC networking - Connector and network details
- Configuration summary - Deployment info
- Monitoring - Logging queries and metrics namespace
- Invoke commands - gcloud commands for testing
- Health check commands - curl commands with auth

Useful for:
- Retrieving service URLs programmatically
- Getting service account emails for IAM setup
- Constructing monitoring queries
- Testing service endpoints

#### provider.tf (1.4 KB)
**Google Cloud provider configuration**

Contains:
- Required provider versions
- Terraform version requirement
- Provider authentication
- Data sources (current GCP account)
- Optional backend configuration for remote state

Features:
- Uses default credentials (gcloud auth login)
- Optional GCS backend for state management
- Both `google` and `google-beta` providers

### Environment Configuration Files

#### terraform.tfvars.example (4.6 KB)
Template with all available configuration options:
- Fully commented for guidance
- Shows recommended values for each option
- Includes environment-specific sections
- Ready to copy and customize

#### terraform.tfvars.dev (1 KB)
**Development environment**
- Lowest resource allocation
- Scale to zero (0 min instances)
- Latest image tags
- All monitoring enabled
- Internal-only access

Optimized for:
- Learning and testing
- Cost efficiency ($10-20/month)
- Fast iteration

#### terraform.tfvars.staging (1.1 KB)
**Staging environment**
- Moderate resource allocation
- Always-on (1 min instance)
- Version-specific image tags
- Standard monitoring
- Public access (IAM-protected)

Optimized for:
- Pre-production testing
- Production parity
- Cost balanced with reliability ($50-100/month)

#### terraform.tfvars.prod (2.1 KB)
**Production environment**
- High resource allocation
- Always-on (2+ min instances)
- Release version tags
- Extended log retention
- Load balancer and CDN enabled

Optimized for:
- High availability (99.99% SLA)
- Performance and reliability
- Complete monitoring and observability ($3k-5k/month)

### Documentation Files

#### QUICKSTART.md (3 KB)
**Get started in 10 minutes**

Perfect for:
- First-time users
- Quick deployments
- Understanding basic commands

Contains:
- 1-minute setup
- 5-minute deployment
- Verification steps
- Key commands summary
- Common tasks

Start here if you're new!

#### README.md (18 KB)
**Complete reference documentation**

Sections:
- Overview and architecture
- File descriptions
- Prerequisites and setup
- Deployment instructions
- Configuration variables
- Monitoring and logging
- Environment-specific deployments
- CI/CD integration
- Troubleshooting
- References and resources

Use for:
- Understanding all features
- Reference on deployment options
- Detailed troubleshooting
- Best practices

#### DEPLOYMENT_GUIDE.md (14 KB)
**Step-by-step deployment instructions**

Sections:
- Detailed prerequisites
- Setup walkthrough (5 main steps)
- GCP project creation
- API enablement
- Container image building
- Terraform initialization
- Deployment for each environment
- Verification procedures
- Creating Dockerfiles
- Post-deployment tasks
- Rollback procedures

Use for:
- First deployment
- Learning the process
- Setting up multiple environments
- Creating Dockerfiles

#### COST_OPTIMIZATION.md (12 KB)
**Cost management and optimization**

Sections:
- Cost estimation tools
- Environment-specific optimization
- Monthly cost breakdowns
- Monitoring and alerting
- Detailed cost calculation
- Cost reduction checklist
- Example cost scenarios

Use for:
- Understanding costs
- Optimizing spending
- Budget planning
- Cost analysis

#### INFRASTRUCTURE_OVERVIEW.md
**This file - Architecture and file guide**

## Resource Summary

### What Gets Created

| Resource | Count | Purpose |
|----------|-------|---------|
| Cloud Run Services | 3 | Edge, Compiler, Marketplace |
| Service Accounts | 3 | One per service |
| IAM Role Bindings | 15+ | Permissions for each service |
| VPC Network | 1 | Custom network (optional) |
| VPC Subnet | 1 | 10.0.0.0/24 (optional) |
| VPC Connector | 1 | Private networking (optional) |
| APIs Enabled | 10+ | Required Google APIs |

### Resource Costs

Monthly costs by environment:

| Environment | Baseline | Peak | Notes |
|-------------|----------|------|-------|
| Development | $1-20 | $50 | Scales to zero |
| Staging | $50 | $100 | 1-10 instances |
| Production | $2,160 | $4,000 | 2-100 instances, HA |

See [COST_OPTIMIZATION.md](./COST_OPTIMIZATION.md) for details.

## Deployment Workflows

### Development

```bash
cd terraform
terraform init
terraform apply -var-file="terraform.tfvars.dev"
```

**Typical flow:**
1. Iterate on code locally
2. Push images to Artifact Registry
3. Deploy with Terraform
4. Test via Cloud Run endpoints
5. Check logs with `make logs`
6. Destroy when done to save costs

### Staging

```bash
terraform apply -var-file="terraform.tfvars.staging"
```

**Typical flow:**
1. Create release candidate images
2. Deploy to staging
3. Run integration tests
4. Monitor for 24-48 hours
5. If successful, promote to production

### Production

```bash
terraform apply -var-file="terraform.tfvars.prod"
```

**Typical flow:**
1. Create release images with version tags
2. Deploy with canary (1-2 instances first)
3. Gradually increase max_instances
4. Monitor metrics and logs closely
5. Keep previous version ready for rollback

## Making Changes

### Update Service Configuration

```bash
# Change max instances for edge service
terraform apply -var-file="terraform.tfvars" \
  -var="edge_max_instances=20"

# Use new image version
terraform apply -var-file="terraform.tfvars" \
  -var="edge_image_tag=v1.0.0"

# Increase memory allocation
terraform apply -var-file="terraform.tfvars" \
  -var="compiler_memory=4Gi"
```

### Add New Service

1. Create new service account in `main.tf`
2. Add IAM role bindings
3. Create Cloud Run service definition
4. Add variables in `variables.tf`
5. Add outputs in `outputs.tf`
6. Update configuration files

### Enable New Feature

Example: Enable Firestore for new service

```hcl
# In main.tf
resource "google_project_iam_member" "new_service_firestore" {
  project = var.project_id
  role    = "roles/datastore.user"
  member  = "serviceAccount:${google_service_account.new_service.email}"
}
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Deploy A2A-RS
on:
  push:
    branches: [main]
    paths: [terraform/**, a2a-*/src/**]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: hashicorp/setup-terraform@v2
      - run: terraform -chdir=terraform init
      - run: terraform -chdir=terraform plan -var-file="terraform.tfvars.prod"
      - run: terraform -chdir=terraform apply -auto-approve -var-file="terraform.tfvars.prod"
```

## Troubleshooting Common Issues

### Service won't start
```bash
# Check logs
make logs ENV=dev

# Verify service account permissions
gcloud projects get-iam-policy PROJECT_ID

# Check image exists
gcloud artifacts docker images list us-central1-docker.pkg.dev/PROJECT_ID/a2a-rs
```

### Terraform apply fails
```bash
# Validate configuration
terraform validate

# Format and check for syntax errors
terraform fmt -recursive

# Check state
terraform state list
```

### High costs
```bash
# Review resource allocation
make output ENV=dev

# Check for orphaned resources
terraform state list

# Reduce max instances or disable VPC connector
```

## Next Steps

1. **For Quick Start**: Read [QUICKSTART.md](./QUICKSTART.md)
2. **For Deployment**: Follow [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md)
3. **For Reference**: See [README.md](./README.md)
4. **For Costs**: Check [COST_OPTIMIZATION.md](./COST_OPTIMIZATION.md)

## Getting Help

1. Check relevant markdown file (README.md, DEPLOYMENT_GUIDE.md, etc.)
2. Review Terraform output: `terraform show`
3. Check logs: `make logs ENV=dev`
4. Review GCP Console: https://console.cloud.google.com/
5. Check Terraform Google Provider docs: https://registry.terraform.io/providers/hashicorp/google/latest

## Version Information

- **Terraform**: >= 1.0
- **Google Provider**: ~> 5.0
- **Google Beta Provider**: ~> 5.0
- **Rust Edition**: 2024
- **Rust MSRV**: 1.85

## Related Documentation

- [A2A-RS Main README](../README.md)
- [CLAUDE.md](../CLAUDE.md) - Project conventions
- [Cloud Run Documentation](https://cloud.google.com/run/docs)
- [Terraform Documentation](https://www.terraform.io/docs)

---

**Ready to deploy?** Start with [QUICKSTART.md](./QUICKSTART.md)
