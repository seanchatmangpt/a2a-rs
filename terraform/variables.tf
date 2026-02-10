# A2A-RS Terraform - Input Variables
# Define all configuration parameters for Cloud Run services

# ==============================================================================
# PROJECT & REGION CONFIGURATION
# ==============================================================================

variable "project_id" {
  description = "GCP Project ID"
  type        = string
  validation {
    condition     = can(regex("^[a-z][a-z0-9-]*[a-z0-9]$", var.project_id))
    error_message = "Project ID must be a valid GCP project identifier."
  }
}

variable "region" {
  description = "GCP region for Cloud Run services"
  type        = string
  default     = "us-central1"
  validation {
    condition     = contains(["us-central1", "us-east1", "us-west1", "us-east4", "europe-west1", "europe-west4", "asia-east1"], var.region)
    error_message = "Region must be a valid GCP Cloud Run region."
  }
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "dev"
  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be dev, staging, or prod."
  }
}

# ==============================================================================
# ARTIFACT REGISTRY CONFIGURATION
# ==============================================================================

variable "artifact_registry_repo" {
  description = "Artifact Registry repository name for container images"
  type        = string
  default     = "a2a-rs"
}

variable "edge_image_tag" {
  description = "Docker image tag for osiris-edge service"
  type        = string
  default     = "latest"
}

variable "compiler_image_tag" {
  description = "Docker image tag for osiris-compiler service"
  type        = string
  default     = "latest"
}

variable "marketplace_image_tag" {
  description = "Docker image tag for osiris-marketplace service"
  type        = string
  default     = "latest"
}

# ==============================================================================
# EDGE SERVICE CONFIGURATION
# ==============================================================================

variable "edge_memory" {
  description = "Memory allocation for osiris-edge service (e.g., '512Mi', '1Gi', '2Gi')"
  type        = string
  default     = "1Gi"
  validation {
    condition     = can(regex("^\\d+(?:Mi|Gi)$", var.edge_memory))
    error_message = "Memory must be in format like '512Mi' or '1Gi'."
  }
}

variable "edge_cpu" {
  description = "CPU allocation for osiris-edge service (e.g., '0.5', '1', '2', '4')"
  type        = string
  default     = "1"
  validation {
    condition     = contains(["0.5", "1", "2", "4"], var.edge_cpu)
    error_message = "CPU must be one of: 0.5, 1, 2, 4."
  }
}

variable "edge_timeout" {
  description = "Request timeout in seconds for osiris-edge"
  type        = number
  default     = 3600
  validation {
    condition     = var.edge_timeout >= 1 && var.edge_timeout <= 3600
    error_message = "Timeout must be between 1 and 3600 seconds."
  }
}

variable "edge_concurrency" {
  description = "Maximum concurrent requests per instance for osiris-edge"
  type        = number
  default     = 100
  validation {
    condition     = var.edge_concurrency >= 1 && var.edge_concurrency <= 1000
    error_message = "Concurrency must be between 1 and 1000."
  }
}

variable "edge_min_instances" {
  description = "Minimum number of instances for osiris-edge"
  type        = number
  default     = 1
  validation {
    condition     = var.edge_min_instances >= 0 && var.edge_min_instances <= 100
    error_message = "Min instances must be between 0 and 100."
  }
}

variable "edge_max_instances" {
  description = "Maximum number of instances for osiris-edge"
  type        = number
  default     = 10
  validation {
    condition     = var.edge_max_instances >= 1 && var.edge_max_instances <= 1000
    error_message = "Max instances must be between 1 and 1000."
  }
}

variable "edge_env_vars" {
  description = "Environment variables for osiris-edge service"
  type        = map(string)
  default = {
    LOG_LEVEL      = "info"
    ENABLE_METRICS = "true"
  }
}

variable "edge_secrets" {
  description = "Secrets for osiris-edge service"
  type = map(object({
    secret  = string
    version = string
  }))
  default = {}
}

# ==============================================================================
# COMPILER SERVICE CONFIGURATION
# ==============================================================================

variable "compiler_memory" {
  description = "Memory allocation for osiris-compiler service"
  type        = string
  default     = "2Gi"
  validation {
    condition     = can(regex("^\\d+(?:Mi|Gi)$", var.compiler_memory))
    error_message = "Memory must be in format like '512Mi' or '1Gi'."
  }
}

variable "compiler_cpu" {
  description = "CPU allocation for osiris-compiler service"
  type        = string
  default     = "2"
  validation {
    condition     = contains(["0.5", "1", "2", "4"], var.compiler_cpu)
    error_message = "CPU must be one of: 0.5, 1, 2, 4."
  }
}

variable "compiler_timeout" {
  description = "Request timeout in seconds for osiris-compiler"
  type        = number
  default     = 3600
  validation {
    condition     = var.compiler_timeout >= 1 && var.compiler_timeout <= 3600
    error_message = "Timeout must be between 1 and 3600 seconds."
  }
}

variable "compiler_concurrency" {
  description = "Maximum concurrent requests per instance for osiris-compiler"
  type        = number
  default     = 50
  validation {
    condition     = var.compiler_concurrency >= 1 && var.compiler_concurrency <= 1000
    error_message = "Concurrency must be between 1 and 1000."
  }
}

variable "compiler_min_instances" {
  description = "Minimum number of instances for osiris-compiler"
  type        = number
  default     = 1
  validation {
    condition     = var.compiler_min_instances >= 0 && var.compiler_min_instances <= 100
    error_message = "Min instances must be between 0 and 100."
  }
}

variable "compiler_max_instances" {
  description = "Maximum number of instances for osiris-compiler"
  type        = number
  default     = 20
  validation {
    condition     = var.compiler_max_instances >= 1 && var.compiler_max_instances <= 1000
    error_message = "Max instances must be between 1 and 1000."
  }
}

variable "compiler_env_vars" {
  description = "Environment variables for osiris-compiler service"
  type        = map(string)
  default = {
    LOG_LEVEL                   = "info"
    ENABLE_METRICS              = "true"
    DETERMINISTIC_ORDERING      = "true"
    PIPELINE_STAGE_TIMEOUT      = "300"
  }
}

variable "compiler_secrets" {
  description = "Secrets for osiris-compiler service"
  type = map(object({
    secret  = string
    version = string
  }))
  default = {}
}

# ==============================================================================
# MARKETPLACE SERVICE CONFIGURATION
# ==============================================================================

variable "marketplace_memory" {
  description = "Memory allocation for osiris-marketplace service"
  type        = string
  default     = "1Gi"
  validation {
    condition     = can(regex("^\\d+(?:Mi|Gi)$", var.marketplace_memory))
    error_message = "Memory must be in format like '512Mi' or '1Gi'."
  }
}

variable "marketplace_cpu" {
  description = "CPU allocation for osiris-marketplace service"
  type        = string
  default     = "1"
  validation {
    condition     = contains(["0.5", "1", "2", "4"], var.marketplace_cpu)
    error_message = "CPU must be one of: 0.5, 1, 2, 4."
  }
}

variable "marketplace_timeout" {
  description = "Request timeout in seconds for osiris-marketplace"
  type        = number
  default     = 3600
  validation {
    condition     = var.marketplace_timeout >= 1 && var.marketplace_timeout <= 3600
    error_message = "Timeout must be between 1 and 3600 seconds."
  }
}

variable "marketplace_concurrency" {
  description = "Maximum concurrent requests per instance for osiris-marketplace"
  type        = number
  default     = 100
  validation {
    condition     = var.marketplace_concurrency >= 1 && var.marketplace_concurrency <= 1000
    error_message = "Concurrency must be between 1 and 1000."
  }
}

variable "marketplace_min_instances" {
  description = "Minimum number of instances for osiris-marketplace"
  type        = number
  default     = 1
  validation {
    condition     = var.marketplace_min_instances >= 0 && var.marketplace_min_instances <= 100
    error_message = "Min instances must be between 0 and 100."
  }
}

variable "marketplace_max_instances" {
  description = "Maximum number of instances for osiris-marketplace"
  type        = number
  default     = 15
  validation {
    condition     = var.marketplace_max_instances >= 1 && var.marketplace_max_instances <= 1000
    error_message = "Max instances must be between 1 and 1000."
  }
}

variable "marketplace_env_vars" {
  description = "Environment variables for osiris-marketplace service"
  type        = map(string)
  default = {
    LOG_LEVEL                     = "info"
    ENABLE_METRICS                = "true"
    ENABLE_SERVICE_CONTROL        = "true"
  }
}

variable "marketplace_secrets" {
  description = "Secrets for osiris-marketplace service"
  type = map(object({
    secret  = string
    version = string
  }))
  default = {}
}

# ==============================================================================
# COMMON CONFIGURATION
# ==============================================================================

variable "common_env_vars" {
  description = "Environment variables common to all services"
  type        = map(string)
  default = {
    ENVIRONMENT = "production"
    RUST_LOG    = "info"
  }
}

variable "common_secrets" {
  description = "Secrets common to all services"
  type = map(object({
    secret  = string
    version = string
  }))
  default = {}
}

variable "health_check_path" {
  description = "HTTP path for health checks"
  type        = string
  default     = "/health"
}

variable "enable_public_ingress" {
  description = "Enable public ingress for Cloud Run services"
  type        = bool
  default     = false
}

# ==============================================================================
# NETWORKING CONFIGURATION
# ==============================================================================

variable "enable_vpc_connector" {
  description = "Enable VPC connector for private networking"
  type        = bool
  default     = false
}

variable "create_custom_network" {
  description = "Create custom VPC network (requires enable_vpc_connector=true)"
  type        = bool
  default     = false
}

variable "network_name" {
  description = "VPC network name"
  type        = string
  default     = "a2a-rs-network"
}

variable "subnet_cidr" {
  description = "CIDR range for the subnet"
  type        = string
  default     = "10.0.0.0/24"
}

variable "vpc_connector_cidr" {
  description = "CIDR range for VPC connector"
  type        = string
  default     = "10.8.0.0/28"
  validation {
    condition     = can(cidrhost(var.vpc_connector_cidr, 0))
    error_message = "VPC connector CIDR must be a valid IPv4 CIDR block."
  }
}

variable "vpc_connector_min_instances" {
  description = "Minimum number of instances for VPC connector"
  type        = number
  default     = 2
  validation {
    condition     = var.vpc_connector_min_instances >= 2 && var.vpc_connector_min_instances <= 10
    error_message = "VPC connector min instances must be between 2 and 10."
  }
}

variable "vpc_connector_max_instances" {
  description = "Maximum number of instances for VPC connector"
  type        = number
  default     = 10
  validation {
    condition     = var.vpc_connector_max_instances >= 2 && var.vpc_connector_max_instances <= 300
    error_message = "VPC connector max instances must be between 2 and 300."
  }
}

# ==============================================================================
# FEATURE FLAGS
# ==============================================================================

variable "enable_firestore" {
  description = "Enable Firestore access for services"
  type        = bool
  default     = true
}

variable "enable_gcs" {
  description = "Enable Google Cloud Storage access for services"
  type        = bool
  default     = true
}

variable "enable_service_control" {
  description = "Enable Service Control API for marketplace service"
  type        = bool
  default     = true
}

variable "enable_load_balancer" {
  description = "Enable external load balancer for edge service"
  type        = bool
  default     = false
}

variable "enable_cdn" {
  description = "Enable CDN for load balanced services"
  type        = bool
  default     = false
}

# ==============================================================================
# MONITORING & OBSERVABILITY
# ==============================================================================

variable "enable_tracing" {
  description = "Enable Cloud Trace integration"
  type        = bool
  default     = true
}

variable "enable_profiler" {
  description = "Enable Cloud Profiler integration"
  type        = bool
  default     = false
}

variable "log_retention_days" {
  description = "Number of days to retain logs"
  type        = number
  default     = 30
  validation {
    condition     = var.log_retention_days > 0
    error_message = "Log retention must be greater than 0."
  }
}

# ==============================================================================
# TAGS & LABELS
# ==============================================================================

variable "tags" {
  description = "Additional labels for all resources"
  type        = map(string)
  default = {
    "team"      = "a2a-rs"
    "project"   = "agent-to-agent"
  }
}
