# A2A-RS Terraform - Provider Configuration
# Google Cloud Platform provider setup

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 5.0"
    }
  }

  # Uncomment to use remote state in Google Cloud Storage
  # backend "gcs" {
  #   bucket = "a2a-rs-terraform-state"
  #   prefix = "a2a-rs"
  # }
}

provider "google" {
  project = var.project_id
  region  = var.region

  # Use default credentials from environment or gcloud
  # To authenticate: gcloud auth application-default login
}

provider "google-beta" {
  project = var.project_id
  region  = var.region
}

# ==============================================================================
# DATA SOURCES
# ==============================================================================

# Get current GCP account for reference
data "google_client_config" "current" {}

# ==============================================================================
# LOCAL VALUES FOR COMMON USE
# ==============================================================================

locals {
  credentials_file = try(var.credentials_file, null)

  # Common provider attributes
  gcp_project = var.project_id
  gcp_region  = var.region
}
