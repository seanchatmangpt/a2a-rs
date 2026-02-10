# A2A-RS Workspace - Cloud Run Services
# Terraform configuration for osiris-edge, osiris-compiler, and osiris-marketplace services

# ==============================================================================
# LOCAL VARIABLES
# ==============================================================================

locals {
  service_image_prefix = "${var.region}-docker.pkg.dev/${var.project_id}/${var.artifact_registry_repo}"

  services = {
    edge = {
      name        = "osiris-edge"
      description = "Edge service: HTTP/WebSocket server for admissions control, WIP gate, refusal engine"
      memory      = var.edge_memory
      cpu         = var.edge_cpu
      timeout     = var.edge_timeout
      image       = "${local.service_image_prefix}/osiris-edge:${var.edge_image_tag}"
      concurrency = var.edge_concurrency
      min_instances = var.edge_min_instances
      max_instances = var.edge_max_instances
      env_vars = merge(var.common_env_vars, var.edge_env_vars)
      secrets  = merge(var.common_secrets, var.edge_secrets)
      health_check = true
      port = 8080
    }
    compiler = {
      name        = "osiris-compiler"
      description = "Compiler service: Deterministic compilation with 7-stage pipeline"
      memory      = var.compiler_memory
      cpu         = var.compiler_cpu
      timeout     = var.compiler_timeout
      image       = "${local.service_image_prefix}/osiris-compiler:${var.compiler_image_tag}"
      concurrency = var.compiler_concurrency
      min_instances = var.compiler_min_instances
      max_instances = var.compiler_max_instances
      env_vars = merge(var.common_env_vars, var.compiler_env_vars)
      secrets  = merge(var.common_secrets, var.compiler_secrets)
      health_check = true
      port = 8080
    }
    marketplace = {
      name        = "osiris-marketplace"
      description = "Marketplace service: Google Workspace integrations, Cloud Service Control usage reporting"
      memory      = var.marketplace_memory
      cpu         = var.marketplace_cpu
      timeout     = var.marketplace_timeout
      image       = "${local.service_image_prefix}/osiris-marketplace:${var.marketplace_image_tag}"
      concurrency = var.marketplace_concurrency
      min_instances = var.marketplace_min_instances
      max_instances = var.marketplace_max_instances
      env_vars = merge(var.common_env_vars, var.marketplace_env_vars)
      secrets  = merge(var.common_secrets, var.marketplace_secrets)
      health_check = true
      port = 8080
    }
  }

  common_labels = {
    project     = "a2a-rs"
    managed_by  = "terraform"
    environment = var.environment
    created_at  = timestamp()
  }
}

# ==============================================================================
# SERVICE ACCOUNTS
# ==============================================================================

# Edge Service Account
resource "google_service_account" "edge" {
  account_id   = "${local.services.edge.name}-sa"
  display_name = "Service account for ${local.services.edge.name}"
  description  = "Runs the osiris-edge Cloud Run service"
  project      = var.project_id
}

# Compiler Service Account
resource "google_service_account" "compiler" {
  account_id   = "${local.services.compiler.name}-sa"
  display_name = "Service account for ${local.services.compiler.name}"
  description  = "Runs the osiris-compiler Cloud Run service"
  project      = var.project_id
}

# Marketplace Service Account
resource "google_service_account" "marketplace" {
  account_id   = "${local.services.marketplace.name}-sa"
  display_name = "Service account for ${local.services.marketplace.name}"
  description  = "Runs the osiris-marketplace Cloud Run service"
  project      = var.project_id
}

# ==============================================================================
# IAM ROLE ASSIGNMENTS - EDGE SERVICE
# ==============================================================================

# Allow edge service to pull images from Artifact Registry
resource "google_project_iam_member" "edge_artifact_reader" {
  project = var.project_id
  role    = "roles/artifactregistry.reader"
  member  = "serviceAccount:${google_service_account.edge.email}"
}

# Allow edge service to write logs
resource "google_project_iam_member" "edge_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.edge.email}"
}

# Allow edge service to write metrics
resource "google_project_iam_member" "edge_metric_writer" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.edge.email}"
}

# Allow edge service to access Firestore (for state/cache if needed)
resource "google_project_iam_member" "edge_firestore_user" {
  count   = var.enable_firestore ? 1 : 0
  project = var.project_id
  role    = "roles/datastore.user"
  member  = "serviceAccount:${google_service_account.edge.email}"
}

# Allow edge service to access GCS (for artifact storage)
resource "google_project_iam_member" "edge_gcs_admin" {
  count   = var.enable_gcs ? 1 : 0
  project = var.project_id
  role    = "roles/storage.objectAdmin"
  member  = "serviceAccount:${google_service_account.edge.email}"
}

# Allow edge service to invoke the compiler service
resource "google_cloud_run_service_iam_member" "edge_invoke_compiler" {
  service  = google_cloud_run_v2_service.compiler.name
  location = var.region
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.edge.email}"
}

# ==============================================================================
# IAM ROLE ASSIGNMENTS - COMPILER SERVICE
# ==============================================================================

# Allow compiler service to pull images from Artifact Registry
resource "google_project_iam_member" "compiler_artifact_reader" {
  project = var.project_id
  role    = "roles/artifactregistry.reader"
  member  = "serviceAccount:${google_service_account.compiler.email}"
}

# Allow compiler service to write logs
resource "google_project_iam_member" "compiler_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.compiler.email}"
}

# Allow compiler service to write metrics
resource "google_project_iam_member" "compiler_metric_writer" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.compiler.email}"
}

# Allow compiler service to access Firestore (for deterministic state)
resource "google_project_iam_member" "compiler_firestore_user" {
  count   = var.enable_firestore ? 1 : 0
  project = var.project_id
  role    = "roles/datastore.user"
  member  = "serviceAccount:${google_service_account.compiler.email}"
}

# Allow compiler service to access GCS (for receipt storage)
resource "google_project_iam_member" "compiler_gcs_admin" {
  count   = var.enable_gcs ? 1 : 0
  project = var.project_id
  role    = "roles/storage.objectAdmin"
  member  = "serviceAccount:${google_service_account.compiler.email}"
}

# ==============================================================================
# IAM ROLE ASSIGNMENTS - MARKETPLACE SERVICE
# ==============================================================================

# Allow marketplace service to pull images from Artifact Registry
resource "google_project_iam_member" "marketplace_artifact_reader" {
  project = var.project_id
  role    = "roles/artifactregistry.reader"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# Allow marketplace service to write logs
resource "google_project_iam_member" "marketplace_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# Allow marketplace service to write metrics
resource "google_project_iam_member" "marketplace_metric_writer" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# Allow marketplace service to use Service Control API for usage reporting
resource "google_project_iam_member" "marketplace_service_control_admin" {
  count   = var.enable_service_control ? 1 : 0
  project = var.project_id
  role    = "roles/servicemanagement.admin"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# Allow marketplace service to access Firestore
resource "google_project_iam_member" "marketplace_firestore_user" {
  count   = var.enable_firestore ? 1 : 0
  project = var.project_id
  role    = "roles/datastore.user"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# Allow marketplace service to access GCS
resource "google_project_iam_member" "marketplace_gcs_admin" {
  count   = var.enable_gcs ? 1 : 0
  project = var.project_id
  role    = "roles/storage.objectAdmin"
  member  = "serviceAccount:${google_service_account.marketplace.email}"
}

# ==============================================================================
# VPC CONNECTOR
# ==============================================================================

resource "google_vpc_access_connector" "a2a_connector" {
  count         = var.enable_vpc_connector ? 1 : 0
  name          = "${var.network_name}-connector"
  provider      = google-beta
  region        = var.region
  ip_cidr_range = var.vpc_connector_cidr
  network       = var.network_name
  min_instances = var.vpc_connector_min_instances
  max_instances = var.vpc_connector_max_instances

  depends_on = [
    google_compute_network.main
  ]
}

# ==============================================================================
# CUSTOM NETWORK (Optional)
# ==============================================================================

resource "google_compute_network" "main" {
  count                   = var.create_custom_network ? 1 : 0
  name                    = var.network_name
  auto_create_subnetworks = false
  project                 = var.project_id

  depends_on = [
    google_project_service.required_apis
  ]
}

resource "google_compute_subnetwork" "main" {
  count         = var.create_custom_network ? 1 : 0
  name          = "${var.network_name}-subnet"
  ip_cidr_range = var.subnet_cidr
  region        = var.region
  network       = google_compute_network.main[0].id
  project       = var.project_id
}

# ==============================================================================
# CLOUD RUN SERVICES
# ==============================================================================

# Edge Service
resource "google_cloud_run_v2_service" "edge" {
  name        = local.services.edge.name
  location    = var.region
  description = local.services.edge.description
  project     = var.project_id

  ingress = var.enable_public_ingress ? "INGRESS_TRAFFIC_ALL" : "INGRESS_TRAFFIC_INTERNAL_ONLY"

  template {
    service_account = google_service_account.edge.email
    timeout         = "${local.services.edge.timeout}s"
    revision        = "${local.services.edge.name}-${formatdate("YYYY-MM-DD-hhmm", timestamp())}"

    max_instance_request_concurrency = local.services.edge.concurrency

    containers {
      image   = local.services.edge.image
      ports {
        container_port = local.services.edge.port
        name           = "http1"
      }

      # Environment variables
      dynamic "env" {
        for_each = local.services.edge.env_vars
        content {
          name  = env.key
          value = env.value
        }
      }

      # Secrets
      dynamic "env" {
        for_each = local.services.edge.secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      # Resource limits
      resources {
        limits = {
          cpu    = local.services.edge.cpu
          memory = local.services.edge.memory
        }
      }

      # Startup probe
      startup_probe {
        http_get {
          path = var.health_check_path
          port = local.services.edge.port
        }
        initial_delay_seconds = 10
        timeout_seconds       = 5
        period_seconds        = 3
        failure_threshold     = 3
      }

      # Liveness probe
      liveness_probe {
        http_get {
          path = var.health_check_path
          port = local.services.edge.port
        }
        initial_delay_seconds = 30
        timeout_seconds       = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # VPC connector if enabled
    vpc_access {
      connector = var.enable_vpc_connector ? google_vpc_access_connector.a2a_connector[0].id : null
      egress    = "PRIVATE_RANGES_ONLY"
    }

    scaling {
      min_instance_count = local.services.edge.min_instances
      max_instance_count = local.services.edge.max_instances
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = merge(
    local.common_labels,
    {
      service = "edge"
    }
  )

  depends_on = [
    google_service_account.edge,
    google_project_iam_member.edge_artifact_reader,
    google_project_iam_member.edge_log_writer,
    google_project_iam_member.edge_metric_writer
  ]
}

# Compiler Service
resource "google_cloud_run_v2_service" "compiler" {
  name        = local.services.compiler.name
  location    = var.region
  description = local.services.compiler.description
  project     = var.project_id

  ingress = var.enable_public_ingress ? "INGRESS_TRAFFIC_ALL" : "INGRESS_TRAFFIC_INTERNAL_ONLY"

  template {
    service_account = google_service_account.compiler.email
    timeout         = "${local.services.compiler.timeout}s"
    revision        = "${local.services.compiler.name}-${formatdate("YYYY-MM-DD-hhmm", timestamp())}"

    max_instance_request_concurrency = local.services.compiler.concurrency

    containers {
      image   = local.services.compiler.image
      ports {
        container_port = local.services.compiler.port
        name           = "http1"
      }

      # Environment variables
      dynamic "env" {
        for_each = local.services.compiler.env_vars
        content {
          name  = env.key
          value = env.value
        }
      }

      # Secrets
      dynamic "env" {
        for_each = local.services.compiler.secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      # Resource limits
      resources {
        limits = {
          cpu    = local.services.compiler.cpu
          memory = local.services.compiler.memory
        }
      }

      # Startup probe
      startup_probe {
        http_get {
          path = var.health_check_path
          port = local.services.compiler.port
        }
        initial_delay_seconds = 10
        timeout_seconds       = 5
        period_seconds        = 3
        failure_threshold     = 3
      }

      # Liveness probe
      liveness_probe {
        http_get {
          path = var.health_check_path
          port = local.services.compiler.port
        }
        initial_delay_seconds = 30
        timeout_seconds       = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # VPC connector if enabled
    vpc_access {
      connector = var.enable_vpc_connector ? google_vpc_access_connector.a2a_connector[0].id : null
      egress    = "PRIVATE_RANGES_ONLY"
    }

    scaling {
      min_instance_count = local.services.compiler.min_instances
      max_instance_count = local.services.compiler.max_instances
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = merge(
    local.common_labels,
    {
      service = "compiler"
    }
  )

  depends_on = [
    google_service_account.compiler,
    google_project_iam_member.compiler_artifact_reader,
    google_project_iam_member.compiler_log_writer,
    google_project_iam_member.compiler_metric_writer
  ]
}

# Marketplace Service
resource "google_cloud_run_v2_service" "marketplace" {
  name        = local.services.marketplace.name
  location    = var.region
  description = local.services.marketplace.description
  project     = var.project_id

  ingress = var.enable_public_ingress ? "INGRESS_TRAFFIC_ALL" : "INGRESS_TRAFFIC_INTERNAL_ONLY"

  template {
    service_account = google_service_account.marketplace.email
    timeout         = "${local.services.marketplace.timeout}s"
    revision        = "${local.services.marketplace.name}-${formatdate("YYYY-MM-DD-hhmm", timestamp())}"

    max_instance_request_concurrency = local.services.marketplace.concurrency

    containers {
      image   = local.services.marketplace.image
      ports {
        container_port = local.services.marketplace.port
        name           = "http1"
      }

      # Environment variables
      dynamic "env" {
        for_each = local.services.marketplace.env_vars
        content {
          name  = env.key
          value = env.value
        }
      }

      # Secrets
      dynamic "env" {
        for_each = local.services.marketplace.secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      # Resource limits
      resources {
        limits = {
          cpu    = local.services.marketplace.cpu
          memory = local.services.marketplace.memory
        }
      }

      # Startup probe
      startup_probe {
        http_get {
          path = var.health_check_path
          port = local.services.marketplace.port
        }
        initial_delay_seconds = 10
        timeout_seconds       = 5
        period_seconds        = 3
        failure_threshold     = 3
      }

      # Liveness probe
      liveness_probe {
        http_get {
          path = var.health_check_path
          port = local.services.marketplace.port
        }
        initial_delay_seconds = 30
        timeout_seconds       = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # VPC connector if enabled
    vpc_access {
      connector = var.enable_vpc_connector ? google_vpc_access_connector.a2a_connector[0].id : null
      egress    = "PRIVATE_RANGES_ONLY"
    }

    scaling {
      min_instance_count = local.services.marketplace.min_instances
      max_instance_count = local.services.marketplace.max_instances
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = merge(
    local.common_labels,
    {
      service = "marketplace"
    }
  )

  depends_on = [
    google_service_account.marketplace,
    google_project_iam_member.marketplace_artifact_reader,
    google_project_iam_member.marketplace_log_writer,
    google_project_iam_member.marketplace_metric_writer
  ]
}

# ==============================================================================
# PUBLIC ACCESS - LOAD BALANCERS (Optional)
# ==============================================================================

# External Load Balancer for Edge Service
resource "google_compute_backend_service" "edge_lb" {
  count                   = var.enable_load_balancer ? 1 : 0
  name                    = "${local.services.edge.name}-lb"
  project                 = var.project_id
  enable_cdn              = var.enable_cdn
  connection_draining_timeout_sec = 300

  custom_request_headers {
    headers = ["X-Client-Region:{client_region}"]
  }

  health_checks = [google_compute_health_check.edge_lb[0].id]
}

resource "google_compute_health_check" "edge_lb" {
  count   = var.enable_load_balancer ? 1 : 0
  name    = "${local.services.edge.name}-health-check"
  project = var.project_id

  http_health_check {
    port        = local.services.edge.port
    request_path = var.health_check_path
  }
}

# ==============================================================================
# REQUIRED APIS
# ==============================================================================

resource "google_project_service" "required_apis" {
  for_each = toset([
    "run.googleapis.com",
    "compute.googleapis.com",
    "artifactregistry.googleapis.com",
    "logging.googleapis.com",
    "monitoring.googleapis.com",
    "cloudresourcemanager.googleapis.com",
    "iam.googleapis.com",
  ])

  project = var.project_id
  service = each.value
  disable_on_destroy = false
}

# Optional APIs
resource "google_project_service" "optional_apis" {
  for_each = toset(concat(
    var.enable_firestore ? ["firestore.googleapis.com"] : [],
    var.enable_gcs ? ["storage.googleapis.com"] : [],
    var.enable_service_control ? ["servicemanagement.googleapis.com"] : [],
    var.enable_vpc_connector ? ["servicenetworking.googleapis.com"] : [],
  ))

  project = var.project_id
  service = each.value
  disable_on_destroy = false
}
