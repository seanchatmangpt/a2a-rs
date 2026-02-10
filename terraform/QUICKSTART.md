# A2A-RS Terraform Quick Start

Get A2A-RS services running on Google Cloud in 10 minutes.

## 1-Minute Setup

```bash
# Navigate to terraform directory
cd /home/user/a2a-rs/terraform

# Initialize Terraform
terraform init

# Copy development config
cp terraform.tfvars.dev terraform.tfvars

# Edit with your GCP project ID
nano terraform.tfvars  # Change project_id = "your-project-id"
```

## 5-Minute Deployment

```bash
# Validate configuration
terraform validate

# Plan deployment
terraform plan -var-file="terraform.tfvars"

# Deploy
terraform apply -var-file="terraform.tfvars"
# Type "yes" to confirm
```

## Verify Deployment

```bash
# Get service endpoints
terraform output services_summary

# Test a service
TOKEN=$(gcloud auth print-identity-token)
curl -H "Authorization: Bearer $TOKEN" $(terraform output -raw edge_service_url)/health
```

## Key Commands

```bash
# Plan changes
make plan ENV=dev

# Deploy
make apply ENV=dev

# View logs
make logs ENV=dev

# Destroy
make destroy ENV=dev

# Health check
make health ENV=dev
```

## File Guide

| File | Purpose |
|------|---------|
| `main.tf` | Cloud Run services, IAM, VPC |
| `variables.tf` | Configuration options |
| `outputs.tf` | Service endpoints & details |
| `provider.tf` | GCP provider setup |
| `terraform.tfvars.dev` | Development config |
| `terraform.tfvars.staging` | Staging config |
| `terraform.tfvars.prod` | Production config |
| `README.md` | Full documentation |
| `DEPLOYMENT_GUIDE.md` | Step-by-step instructions |
| `COST_OPTIMIZATION.md` | Cost management |
| `Makefile` | Convenient shortcuts |

## Environment Configs

### Development
```bash
terraform apply -var-file="terraform.tfvars.dev"
```
- Min CPU/memory
- Scale to zero when idle
- Low costs ($10-20/month)

### Staging
```bash
terraform apply -var-file="terraform.tfvars.staging"
```
- Moderate resources
- Always-on (1 instance)
- Medium costs ($50-100/month)

### Production
```bash
terraform apply -var-file="terraform.tfvars.prod"
```
- High CPU/memory
- High availability (2+ instances)
- Higher costs ($3k-5k/month)

## Common Tasks

### Update Service Configuration

```bash
# Scale up compiler service
terraform apply -var-file="terraform.tfvars" \
  -var="compiler_max_instances=50"

# Use newer image
terraform apply -var-file="terraform.tfvars" \
  -var="edge_image_tag=v0.2.0"

# Change region
terraform apply -var-file="terraform.tfvars" \
  -var="region=us-east1"
```

### View Current State

```bash
# List all resources
make state-list

# Show specific resource
make state-show RESOURCE='google_cloud_run_v2_service.edge'

# Show all outputs
make output ENV=dev

# Show as JSON
make output-json ENV=dev
```

### Monitor Services

```bash
# View logs
make logs ENV=dev

# View errors
make logs-errors ENV=dev

# Health check all services
make health ENV=dev

# View specific service logs
make logs-edge ENV=dev
make logs-compiler ENV=dev
make logs-marketplace ENV=dev
```

## Prerequisites Checklist

- [ ] Google Cloud SDK installed
- [ ] Terraform >= 1.0 installed
- [ ] Docker installed
- [ ] GCP project created
- [ ] APIs enabled (see DEPLOYMENT_GUIDE.md)
- [ ] Container images built and pushed
- [ ] `terraform init` run successfully

## Troubleshooting

### "No valid credential"
```bash
gcloud auth application-default login
```

### "Project not found"
```bash
gcloud config set project your-project-id
gcloud config list
```

### "Image not found"
```bash
gcloud artifacts docker images list us-central1-docker.pkg.dev/your-project/a2a-rs
```

### "Service won't start"
```bash
make logs ENV=dev
```

## Next Steps

1. Read [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md) for detailed setup
2. Review [README.md](./README.md) for full documentation
3. Check [COST_OPTIMIZATION.md](./COST_OPTIMIZATION.md) to manage costs
4. Grant access to team members:
   ```bash
   gcloud run services add-iam-policy-binding osiris-edge \
     --member=user:email@example.com \
     --role=roles/run.invoker \
     --region=us-central1
   ```

## Resources

- [Cloud Run Documentation](https://cloud.google.com/run/docs)
- [Terraform Google Provider](https://registry.terraform.io/providers/hashicorp/google/latest)
- [GCP Free Tier](https://cloud.google.com/free)
- [A2A-RS GitHub](https://github.com/your-org/a2a-rs)

## Support

- Questions? See [README.md](./README.md#troubleshooting)
- Cost questions? See [COST_OPTIMIZATION.md](./COST_OPTIMIZATION.md)
- Deployment issues? See [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md#troubleshooting)

---

**Deploy now:**
```bash
terraform init && terraform apply -var-file="terraform.tfvars.dev"
```
