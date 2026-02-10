# A2A-RS Terraform Makefile
# Convenient shortcuts for common Terraform operations

.PHONY: help init plan apply destroy fmt validate clean

# Default environment
ENV ?= dev
TFVARS = terraform.tfvars.$(ENV)
TF_DIR = .
TF = terraform -chdir=$(TF_DIR)

help:
	@echo "A2A-RS Terraform Commands"
	@echo "=========================="
	@echo ""
	@echo "Usage: make [command] ENV=<env>"
	@echo ""
	@echo "Environments: dev, staging, prod (default: dev)"
	@echo ""
	@echo "Commands:"
	@echo "  init              - Initialize Terraform (required first)"
	@echo "  validate          - Validate Terraform configuration"
	@echo "  fmt               - Format Terraform files"
	@echo "  plan              - Show planned changes"
	@echo "  apply             - Apply configuration (requires confirmation)"
	@echo "  apply-auto        - Apply without confirmation (use with caution)"
	@echo "  destroy           - Destroy all resources (requires confirmation)"
	@echo "  destroy-auto      - Destroy without confirmation (DANGEROUS)"
	@echo "  output            - Show all outputs"
	@echo "  state-list        - List resources in state"
	@echo "  state-show        - Show detailed resource info"
	@echo "  logs              - View Cloud Logs for all services"
	@echo "  costs             - Estimate infrastructure costs"
	@echo "  clean             - Remove local Terraform files"
	@echo ""
	@echo "Examples:"
	@echo "  make plan ENV=dev"
	@echo "  make apply ENV=staging"
	@echo "  make destroy ENV=prod"
	@echo "  make output ENV=dev"

.check-env:
	@if [ ! -f "$(TFVARS)" ]; then \
		echo "Error: $(TFVARS) not found"; \
		echo "Available configs:"; \
		ls -1 terraform.tfvars.* 2>/dev/null || echo "  (none)"; \
		exit 1; \
	fi

init:
	$(TF) init

validate: .check-env
	$(TF) validate

fmt:
	$(TF) fmt -recursive

plan: .check-env validate
	$(TF) plan -var-file="$(TFVARS)" -out=tfplan

apply: .check-env plan
	$(TF) apply tfplan
	@echo ""
	@echo "Apply complete! Showing outputs:"
	@make output ENV=$(ENV)

apply-auto: .check-env validate
	$(TF) apply -auto-approve -var-file="$(TFVARS)"
	@echo ""
	@echo "Apply complete! Showing outputs:"
	@make output ENV=$(ENV)

destroy: .check-env
	@echo "WARNING: This will destroy all $(ENV) resources"
	@read -p "Continue? [y/N] " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		$(TF) destroy -var-file="$(TFVARS)"; \
	else \
		echo "Cancelled"; \
	fi

destroy-auto: .check-env
	@echo "ERROR: Use 'make destroy' with confirmation"
	@exit 1

output: .check-env
	$(TF) output -var-file="$(TFVARS)"

output-json: .check-env
	$(TF) output -json -var-file="$(TFVARS)"

state-list: .check-env
	$(TF) state list

state-show:
	@echo "Usage: make state-show RESOURCE=<resource>"
	@echo "Example: make state-show RESOURCE='google_cloud_run_v2_service.edge'"

state-show:
	@if [ -z "$(RESOURCE)" ]; then \
		echo "Error: RESOURCE not specified"; \
		echo "Usage: make state-show RESOURCE=google_cloud_run_v2_service.edge"; \
		exit 1; \
	fi
	$(TF) state show $(RESOURCE)

# Logging and monitoring
logs:
	@echo "Tailing logs for all a2a-rs services..."
	gcloud logs read \
		'resource.type="cloud_run_revision" AND resource.labels.service_name=~"(osiris-edge|osiris-compiler|osiris-marketplace)"' \
		--limit 50 \
		--format='table(timestamp,resource.labels.service_name,jsonPayload.message)'

logs-edge:
	gcloud logs read \
		'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-edge"' \
		--limit 50 \
		--format='table(timestamp,severity,jsonPayload.message)'

logs-compiler:
	gcloud logs read \
		'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-compiler"' \
		--limit 50 \
		--format='table(timestamp,severity,jsonPayload.message)'

logs-marketplace:
	gcloud logs read \
		'resource.type="cloud_run_revision" AND resource.labels.service_name="osiris-marketplace"' \
		--limit 50 \
		--format='table(timestamp,severity,jsonPayload.message)'

logs-errors:
	gcloud logs read \
		'resource.type="cloud_run_revision" AND resource.labels.service_name=~"(osiris-edge|osiris-compiler|osiris-marketplace)" AND severity=ERROR' \
		--limit 50 \
		--format='table(timestamp,resource.labels.service_name,jsonPayload.message)'

# Cost estimation
costs:
	@echo "Estimating infrastructure costs for $(ENV) environment..."
	@echo "Note: Use Infracost for detailed cost analysis"
	@echo "Install: https://www.infracost.io/docs/getting_started/cli_setup/"
	@if command -v infracost > /dev/null; then \
		infracost breakdown --path=$(TF_DIR) --format json --var-file=$(TFVARS); \
	else \
		echo "Infracost not installed. Install from https://www.infracost.io/"; \
	fi

# Health checks
health: .check-env
	@echo "Running health checks for $(ENV) environment..."
	@TOKEN=$$(gcloud auth print-identity-token); \
	EDGE_URL=$$($(TF) output -raw edge_service_url); \
	COMPILER_URL=$$($(TF) output -raw compiler_service_url); \
	MARKETPLACE_URL=$$($(TF) output -raw marketplace_service_url); \
	echo "Edge service: $$EDGE_URL"; \
	curl -s -H "Authorization: Bearer $$TOKEN" "$$EDGE_URL/health" | jq . || echo "FAILED"; \
	echo "Compiler service: $$COMPILER_URL"; \
	curl -s -H "Authorization: Bearer $$TOKEN" "$$COMPILER_URL/health" | jq . || echo "FAILED"; \
	echo "Marketplace service: $$MARKETPLACE_URL"; \
	curl -s -H "Authorization: Bearer $$TOKEN" "$$MARKETPLACE_URL/health" | jq . || echo "FAILED"

# Cleanup
clean:
	rm -rf .terraform/
	rm -f .terraform.lock.hcl
	rm -f tfplan
	rm -f tfplan.json
	@echo "Cleaned local Terraform files"

# Development helpers
graph:
	$(TF) graph > graph.dot
	@echo "Saved dependency graph to graph.dot"
	@echo "Visualize with: dot -Tsvg graph.dot -o graph.svg"

show: .check-env
	$(TF) show -var-file="$(TFVARS)"

import:
	@echo "Usage: make import ENV=<env> RESOURCE=<resource> ID=<id>"
	@echo "Example: make import ENV=dev RESOURCE='google_cloud_run_v2_service.edge' ID='projects/my-project/locations/us-central1/services/osiris-edge'"

import: .check-env
	@if [ -z "$(RESOURCE)" ] || [ -z "$(ID)" ]; then \
		echo "Error: RESOURCE and ID required"; \
		exit 1; \
	fi
	$(TF) import -var-file="$(TFVARS)" $(RESOURCE) $(ID)

# Docker image operations
build-images:
	@echo "Building container images for all services..."
	docker build -t us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-edge:latest \
		-f ../osiris-edge/Dockerfile ..
	docker build -t us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-compiler:latest \
		-f ../osiris-compiler/Dockerfile ..
	docker build -t us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-marketplace:latest \
		-f ../osiris-marketplace/Dockerfile ..
	@echo "Images built successfully"

push-images:
	@echo "Pushing images to Artifact Registry..."
	docker push us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-edge:latest
	docker push us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-compiler:latest
	docker push us-central1-docker.pkg.dev/my-project/a2a-rs/osiris-marketplace:latest
	@echo "Images pushed successfully"

# GCP operations
gcp-info:
	@echo "Current GCP configuration:"
	gcloud config list
	@echo ""
	@echo "Available projects:"
	gcloud projects list | grep a2a-rs || echo "No a2a-rs projects found"

list-services:
	gcloud run services list --region=us-central1 --filter="labels.project=a2a-rs"

describe-edge:
	gcloud run services describe osiris-edge --region=us-central1 --format=json | jq .

describe-compiler:
	gcloud run services describe osiris-compiler --region=us-central1 --format=json | jq .

describe-marketplace:
	gcloud run services describe osiris-marketplace --region=us-central1 --format=json | jq .

# Lifecycle
version:
	$(TF) version

upgrade:
	$(TF) init -upgrade

.PHONY: help init validate fmt plan apply apply-auto destroy output output-json \
        state-list state-show logs logs-edge logs-compiler logs-marketplace logs-errors \
        costs health clean graph show import build-images push-images gcp-info \
        list-services describe-edge describe-compiler describe-marketplace version upgrade
