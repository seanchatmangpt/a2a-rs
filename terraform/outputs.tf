# A2A-RS Terraform - Output Values
# Export important resource identifiers and endpoints

# ==============================================================================
# CLOUD RUN SERVICES - ENDPOINTS
# ==============================================================================

output "edge_service_url" {
  description = "HTTPS endpoint for osiris-edge Cloud Run service"
  value       = google_cloud_run_v2_service.edge.uri
}

output "compiler_service_url" {
  description = "HTTPS endpoint for osiris-compiler Cloud Run service"
  value       = google_cloud_run_v2_service.compiler.uri
}

output "marketplace_service_url" {
  description = "HTTPS endpoint for osiris-marketplace Cloud Run service"
  value       = google_cloud_run_v2_service.marketplace.uri
}

# ==============================================================================
# CLOUD RUN SERVICES - NAMES & IDS
# ==============================================================================

output "edge_service_name" {
  description = "Name of the osiris-edge Cloud Run service"
  value       = google_cloud_run_v2_service.edge.name
}

output "edge_service_revision" {
  description = "Latest revision of osiris-edge service"
  value       = google_cloud_run_v2_service.edge.template[0].revision
}

output "compiler_service_name" {
  description = "Name of the osiris-compiler Cloud Run service"
  value       = google_cloud_run_v2_service.compiler.name
}

output "compiler_service_revision" {
  description = "Latest revision of osiris-compiler service"
  value       = google_cloud_run_v2_service.compiler.template[0].revision
}

output "marketplace_service_name" {
  description = "Name of the osiris-marketplace Cloud Run service"
  value       = google_cloud_run_v2_service.marketplace.name
}

output "marketplace_service_revision" {
  description = "Latest revision of osiris-marketplace service"
  value       = google_cloud_run_v2_service.marketplace.template[0].revision
}

# ==============================================================================
# SERVICE ACCOUNTS
# ==============================================================================

output "edge_service_account_email" {
  description = "Email of the service account for osiris-edge"
  value       = google_service_account.edge.email
}

output "compiler_service_account_email" {
  description = "Email of the service account for osiris-compiler"
  value       = google_service_account.compiler.email
}

output "marketplace_service_account_email" {
  description = "Email of the service account for osiris-marketplace"
  value       = google_service_account.marketplace.email
}

output "service_accounts" {
  description = "All service account details"
  value = {
    edge = {
      email = google_service_account.edge.email
      id    = google_service_account.edge.unique_id
    }
    compiler = {
      email = google_service_account.compiler.email
      id    = google_service_account.compiler.unique_id
    }
    marketplace = {
      email = google_service_account.marketplace.email
      id    = google_service_account.marketplace.unique_id
    }
  }
}

# ==============================================================================
# VPC NETWORKING
# ==============================================================================

output "vpc_connector_id" {
  description = "VPC connector ID for internal networking"
  value       = var.enable_vpc_connector ? google_vpc_access_connector.a2a_connector[0].id : null
}

output "vpc_connector_name" {
  description = "VPC connector name"
  value       = var.enable_vpc_connector ? google_vpc_access_connector.a2a_connector[0].name : null
}

output "network_id" {
  description = "VPC network ID"
  value       = var.create_custom_network ? google_compute_network.main[0].id : null
}

output "network_name" {
  description = "VPC network name"
  value       = var.create_custom_network ? google_compute_network.main[0].name : null
}

output "subnet_id" {
  description = "Subnet ID"
  value       = var.create_custom_network ? google_compute_subnetwork.main[0].id : null
}

output "subnet_name" {
  description = "Subnet name"
  value       = var.create_custom_network ? google_compute_subnetwork.main[0].name : null
}

# ==============================================================================
# CONFIGURATION SUMMARY
# ==============================================================================

output "services_summary" {
  description = "Summary of all deployed services"
  value = {
    edge = {
      name        = google_cloud_run_v2_service.edge.name
      url         = google_cloud_run_v2_service.edge.uri
      location    = google_cloud_run_v2_service.edge.location
      service_account = google_service_account.edge.email
      cpu         = google_cloud_run_v2_service.edge.template[0].containers[0].resources[0].limits.cpu
      memory      = google_cloud_run_v2_service.edge.template[0].containers[0].resources[0].limits.memory
      min_instances = google_cloud_run_v2_service.edge.template[0].scaling[0].min_instance_count
      max_instances = google_cloud_run_v2_service.edge.template[0].scaling[0].max_instance_count
    }
    compiler = {
      name        = google_cloud_run_v2_service.compiler.name
      url         = google_cloud_run_v2_service.compiler.uri
      location    = google_cloud_run_v2_service.compiler.location
      service_account = google_service_account.compiler.email
      cpu         = google_cloud_run_v2_service.compiler.template[0].containers[0].resources[0].limits.cpu
      memory      = google_cloud_run_v2_service.compiler.template[0].containers[0].resources[0].limits.memory
      min_instances = google_cloud_run_v2_service.compiler.template[0].scaling[0].min_instance_count
      max_instances = google_cloud_run_v2_service.compiler.template[0].scaling[0].max_instance_count
    }
    marketplace = {
      name        = google_cloud_run_v2_service.marketplace.name
      url         = google_cloud_run_v2_service.marketplace.uri
      location    = google_cloud_run_v2_service.marketplace.location
      service_account = google_service_account.marketplace.email
      cpu         = google_cloud_run_v2_service.marketplace.template[0].containers[0].resources[0].limits.cpu
      memory      = google_cloud_run_v2_service.marketplace.template[0].containers[0].resources[0].limits.memory
      min_instances = google_cloud_run_v2_service.marketplace.template[0].scaling[0].min_instance_count
      max_instances = google_cloud_run_v2_service.marketplace.template[0].scaling[0].max_instance_count
    }
  }
}

output "deployment_info" {
  description = "Deployment information"
  value = {
    project_id     = var.project_id
    region         = var.region
    environment    = var.environment
    vpc_enabled    = var.enable_vpc_connector
    public_ingress = var.enable_public_ingress
    timestamp      = timestamp()
  }
}

# ==============================================================================
# MONITORING & OBSERVABILITY
# ==============================================================================

output "logging_bucket" {
  description = "Cloud Logging bucket for service logs"
  value       = "projects/${var.project_id}/locations/${var.region}/buckets/cloud-run-services"
}

output "metrics_namespace" {
  description = "Cloud Monitoring custom metrics namespace"
  value       = "custom.googleapis.com/a2a-rs"
}

output "service_logs_query" {
  description = "Cloud Logging query for all a2a-rs services"
  value       = "resource.type=\"cloud_run_revision\" AND resource.labels.service_name=~\"(osiris-edge|osiris-compiler|osiris-marketplace)\""
}

output "edge_logs_query" {
  description = "Cloud Logging query for osiris-edge service"
  value       = "resource.type=\"cloud_run_revision\" AND resource.labels.service_name=\"${google_cloud_run_v2_service.edge.name}\""
}

output "compiler_logs_query" {
  description = "Cloud Logging query for osiris-compiler service"
  value       = "resource.type=\"cloud_run_revision\" AND resource.labels.service_name=\"${google_cloud_run_v2_service.compiler.name}\""
}

output "marketplace_logs_query" {
  description = "Cloud Logging query for osiris-marketplace service"
  value       = "resource.type=\"cloud_run_revision\" AND resource.labels.service_name=\"${google_cloud_run_v2_service.marketplace.name}\""
}

# ==============================================================================
# INVOKE COMMANDS
# ==============================================================================

output "invoke_commands" {
  description = "gcloud commands to invoke each service"
  value = {
    edge = "gcloud run services call ${google_cloud_run_v2_service.edge.name} --region=${var.region}"
    compiler = "gcloud run services call ${google_cloud_run_v2_service.compiler.name} --region=${var.region}"
    marketplace = "gcloud run services call ${google_cloud_run_v2_service.marketplace.name} --region=${var.region}"
  }
}

output "curl_health_checks" {
  description = "curl commands to test health endpoints"
  value = {
    edge = "curl -H 'Authorization: Bearer $(gcloud auth print-identity-token)' '${google_cloud_run_v2_service.edge.uri}/health'"
    compiler = "curl -H 'Authorization: Bearer $(gcloud auth print-identity-token)' '${google_cloud_run_v2_service.compiler.uri}/health'"
    marketplace = "curl -H 'Authorization: Bearer $(gcloud auth print-identity-token)' '${google_cloud_run_v2_service.marketplace.uri}/health'"
  }
}

# ==============================================================================
# DEPLOYMENT INSTRUCTIONS
# ==============================================================================

output "next_steps" {
  description = "Next steps after deployment"
  value = <<-EOT
1. Deploy container images to Artifact Registry:
   - gcloud builds submit --tag ${var.region}-docker.pkg.dev/${var.project_id}/${var.artifact_registry_repo}/osiris-edge:latest
   - gcloud builds submit --tag ${var.region}-docker.pkg.dev/${var.project_id}/${var.artifact_registry_repo}/osiris-compiler:latest
   - gcloud builds submit --tag ${var.region}-docker.pkg.dev/${var.project_id}/${var.artifact_registry_repo}/osiris-marketplace:latest

2. Verify services are running:
   - gcloud run services list --region=${var.region}

3. Check logs:
   - gcloud logs read --filter='resource.type="cloud_run_revision"' --limit 50

4. Monitor services:
   - Open Cloud Console: https://console.cloud.google.com/run?project=${var.project_id}

5. Set up IAM permissions for users to invoke services:
   - gcloud run services add-iam-policy-binding osiris-edge --member=user:email@example.com --role=roles/run.invoker --region=${var.region}

6. Configure secrets in Secret Manager if needed:
   - gcloud secrets create my-secret --replication-policy="automatic"
   - Reference in Terraform: edge_secrets = { "MY_SECRET" = { secret = "my-secret", version = "latest" } }
  EOT
}
