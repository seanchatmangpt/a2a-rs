# A2A-RS Cost Optimization Guide

Best practices and strategies for optimizing infrastructure costs on Google Cloud.

## Table of Contents

1. [Cost Estimation](#cost-estimation)
2. [Development Environment Optimization](#development-environment-optimization)
3. [Staging Environment Optimization](#staging-environment-optimization)
4. [Production Cost Management](#production-cost-management)
5. [Monitoring & Alerts](#monitoring--alerts)
6. [Cost Breakdown](#cost-breakdown)

## Cost Estimation

### Using Infracost

Install and use Infracost for detailed cost estimates:

```bash
# Install Infracost
curl https://raw.githubusercontent.com/infracost/infracost/master/scripts/install.sh | sh

# Estimate costs
cd terraform
infracost breakdown --path . --var-file terraform.tfvars.prod

# Compare before and after changes
infracost diff --path . --var-file terraform.tfvars.dev --var-file terraform.tfvars.prod
```

### Using Terraform Estimates

```bash
# Show current state costs (requires infracost config)
make costs ENV=prod
```

### Monthly Cost Estimates

Based on typical usage patterns:

| Environment | CPU Hours | Memory Hours | Est. Monthly Cost |
|------------|-----------|--------------|-------------------|
| Development | 100 | 200 | $15-25 |
| Staging | 500 | 1,000 | $50-75 |
| Production | 2,000 | 4,000 | $200-300 |

## Development Environment Optimization

Development environments should prioritize cost efficiency.

### Configuration

```hcl
# terraform.tfvars.dev - Cost-optimized development

# Edge Service
edge_memory        = "512Mi"      # Minimum viable
edge_cpu           = "0.5"        # Lowest CPU tier
edge_min_instances = 0            # Scale to zero when idle
edge_max_instances = 2            # Low maximum

# Compiler Service
compiler_memory        = "1Gi"    # Modest allocation
compiler_cpu           = "1"
compiler_min_instances = 0        # Scale to zero
compiler_max_instances = 2        # Low maximum

# Marketplace Service
marketplace_memory        = "512Mi"
marketplace_cpu           = "0.5"
marketplace_min_instances = 0
marketplace_max_instances = 2

# Disable expensive features
enable_vpc_connector  = false     # VPC connector costs $0.025/hour
enable_load_balancer  = false     # Load balancer costs
enable_cdn            = false     # CDN bandwidth costs
enable_profiler       = false     # Cloud Profiler costs

# Use longer retention for logs
log_retention_days = 7            # Default is 30 days
```

### Cost Savings Strategies

1. **Scale to Zero**
   - Set `min_instances = 0` for automatic scale-down when idle
   - Saves ~70% of baseline costs during low-traffic periods
   - Trade-off: 5-30 second cold start latency

2. **Reduce Resource Allocation**
   ```hcl
   # Instead of 1Gi, use 512Mi
   # Instead of 1 CPU, use 0.5 CPU
   # Only use what you test
   ```

3. **Disable Unnecessary Features**
   - VPC Connector: Costs $0.025/hour when enabled
   - Load Balancer: Costs for each backend service
   - CDN: Bandwidth costs for caching

4. **Shared Resources**
   - Use one VPC for all environments
   - Share Cloud Storage buckets
   - Use shared databases instead of per-service

### Expected Monthly Costs

- **Zero traffic**: $1-5 (just for APIs and storage)
- **Light development**: $10-20
- **Active development**: $25-50

## Staging Environment Optimization

Staging should balance cost with production parity.

### Configuration

```hcl
# terraform.tfvars.staging - Balanced cost

# Moderate resource allocation
edge_memory        = "1Gi"
edge_cpu           = "1"
edge_min_instances = 1           # Always on for testing
edge_max_instances = 5           # Limited scaling

# Keep important features enabled
enable_vpc_connector = true
enable_tracing       = true

# Standard monitoring
log_retention_days = 30          # Standard retention
```

### Cost Optimization

1. **Minimum Instance Count = 1**
   - Ensures service is always ready
   - Costs $0.08/month per service (baseline)
   - Recommended for staging

2. **Limited Auto-Scaling**
   - Max 5-10 instances during peak
   - Prevents runaway costs from traffic spikes
   - Saves 60-80% vs unlimited scaling

3. **Selective Feature Enablement**
   - VPC Connector: Only if network isolation needed
   - Load Balancer: Not needed for staging
   - CDN: Not needed for internal testing

### Expected Monthly Costs

- **Per service, baseline**: ~$0.08
- **Three services, always-on**: ~$0.25
- **With moderate scaling (3 instances avg)**: $30-50
- **With VPC connector**: Add $18/month

## Production Cost Management

Production requires high availability but with cost consciousness.

### Configuration

```hcl
# terraform.tfvars.prod - High availability with cost control

# High resource allocation with limits
edge_memory        = "2Gi"
edge_cpu           = "2"
edge_min_instances = 2           # Always-on for HA
edge_max_instances = 50          # Reasonable limit

# VPC for security, but optimized
enable_vpc_connector  = true
vpc_connector_max_instances = 20  # Cap at 20, not 300

# CDN for popular content
enable_cdn = true                 # Caches responses

# Standard monitoring
log_retention_days = 90
enable_profiler = true            # For optimization
```

### Cost Management Strategies

1. **Always-On Baseline**
   ```hcl
   edge_min_instances       = 2
   compiler_min_instances   = 2
   marketplace_min_instances = 2
   # Cost: ~$0.25/month × 3 × 2,880 hours = ~$2,160/month
   ```

2. **Limit Maximum Instances**
   ```hcl
   edge_max_instances       = 50    # Not 100
   compiler_max_instances   = 100   # Not unlimited
   marketplace_max_instances = 30   # Based on expected peak
   # Prevents surprise costs from traffic spikes
   ```

3. **Enable CDN Selectively**
   ```hcl
   enable_cdn = true
   # Caches responses, reducing backend requests
   # ROI: Positive if 30%+ of requests are cacheable
   ```

4. **Use VPC Connector Efficiently**
   ```hcl
   vpc_connector_min_instances = 3
   vpc_connector_max_instances = 20
   # Connector costs $0.025/hour per instance
   # 24/7: 3 instances × $0.025 × 730 hours = $54.75/month
   ```

5. **Archive Old Logs**
   ```bash
   # Move logs to Cloud Storage after 90 days
   gcloud logging buckets create archive-bucket \
     --bucket-name=a2a-rs-logs-archive \
     --retention-days=365 \
     --location=us
   ```

### Expected Monthly Costs

| Component | Cost | Notes |
|-----------|------|-------|
| Baseline (3 services × 2 instances) | $2,160 | Always-on instances |
| Auto-scaling overage (peak load) | $800-1,200 | Variable, depends on traffic |
| VPC Connector (avg 5 instances) | $91.25 | 5 × $0.025 × 730 hours |
| CDN (if enabled) | $100-500 | Depends on cache hit ratio |
| Storage (Firestore, GCS) | $50-200 | Depends on data volume |
| Logging & Monitoring | $50-100 | Standard ops |
| **Total** | **$3,250-4,760** | Typical production |

## Monitoring & Alerts

### Set Up Budget Alerts

```bash
# Create budget alert
gcloud billing budgets create \
  --billing-account=BILLING_ACCOUNT_ID \
  --display-name="A2A-RS Monthly Budget" \
  --budget-amount=5000 \
  --threshold-rule=percent=50 \
  --threshold-rule=percent=75 \
  --threshold-rule=percent=90 \
  --threshold-rule=percent=100 \
  --threshold-rule=percent=110
```

### Track Costs in Console

1. Open Cloud Billing: https://console.cloud.google.com/billing
2. Select project
3. View **Cost Analysis**:
   - Filter by Service
   - Filter by Resource
   - Group by Region
   - Group by SKU

### Create Cost Report

```bash
# Export daily costs to BigQuery
gcloud billing accounts list
gcloud billing accounts export-daily-cost \
  --billing-account=BILLING_ACCOUNT_ID \
  --destination-dataset=cost_reports
```

### Monitor Service Costs

```bash
# View costs by service
gcloud billing accounts list-cost-elements \
  --billing-account=BILLING_ACCOUNT_ID \
  --format='table(service,cost)'

# View costs by Cloud Run
gcloud compute instances list --format='table(name,zone)'
```

## Cost Breakdown

### CPU Costs

Cloud Run CPU pricing (as of 2024):

| Tier | Price/vCPU-hour |
|------|-----------------|
| First 180,000 vCPU-hours/month | $0.0000247 |
| Beyond 180,000 vCPU-hours/month | $0.0000197 |

Example calculation:
- 1 vCPU × 730 hours/month = 730 vCPU-hours = $0.018

### Memory Costs

Cloud Run memory pricing:

| Tier | Price/GB-hour |
|------|---------------|
| First 360,000 GB-hours/month | $0.0000050 |
| Beyond 360,000 GB-hours/month | $0.0000040 |

Example calculation:
- 1 GB × 730 hours/month = 730 GB-hours = $0.0037

### Request Costs

Cloud Run request pricing:

| Tier | Price/1M requests |
|------|------------------|
| First 2M requests/month | Free |
| Additional requests | $0.40 |

Example:
- 1M requests/month = Free
- 3M requests/month = 1M × $0.40 = $0.40

### VPC Connector Costs

| Component | Cost |
|-----------|------|
| Per instance/hour | $0.025 |
| Minimum instances | 2 × $0.025 × 730 = $36.50/month |
| Per additional instance | $18.25/month |

### Load Balancer Costs

- Per forwarding rule: $0.025/hour = $18/month
- Per backend service: $0.025/hour = $18/month
- Data processing: $0.12 per GB

### CDN Costs

| Region | Price/GB |
|--------|----------|
| Americas | $0.085 |
| Europe | $0.085 |
| Asia | $0.110 |
| Australia | $0.180 |

## Cost Reduction Checklist

### Immediate Savings (Do Today)

- [ ] Set `min_instances = 0` in development
- [ ] Disable VPC connector if not needed ($18-36/month savings)
- [ ] Disable load balancer if not needed ($36+/month savings)
- [ ] Disable profiler if not active ($0-20/month savings)
- [ ] Set appropriate log retention (7 days dev, 30 days staging)

### Short-term Improvements (This Week)

- [ ] Reduce resource allocation to minimum viable
- [ ] Cap maximum instances to reasonable limits
- [ ] Review and remove unused resources
- [ ] Enable CDN if traffic pattern supports it
- [ ] Set up budget alerts

### Medium-term Optimization (This Month)

- [ ] Implement caching strategies to reduce requests
- [ ] Archive logs older than 30 days
- [ ] Consolidate services where possible
- [ ] Review and optimize database queries
- [ ] Monitor cost trends

### Long-term Strategy (Quarterly)

- [ ] Negotiate enterprise discount
- [ ] Consider reserved capacity
- [ ] Evaluate alternative services (GKE, App Engine)
- [ ] Plan for growth and scaling costs
- [ ] Implement FinOps practices

## Example Cost Scenarios

### Small Startup (Dev Only)

```hcl
environment = "dev"
enable_vpc_connector = false
enable_load_balancer = false
edge_min_instances = 0
```

Monthly cost: $10-20

### Growing Company (Dev + Staging)

```hcl
# Dev: Min instances 0
# Staging: Min instances 1, Max 10
```

Monthly cost: $50-100

### Enterprise (Dev + Staging + Prod)

```hcl
# Dev: Min 0, Max 2
# Staging: Min 1, Max 10
# Prod: Min 2, Max 100 (with load balancer)
```

Monthly cost: $3,000-5,000

## References

- [Cloud Run Pricing](https://cloud.google.com/run/pricing)
- [Cloud VPC Connector Pricing](https://cloud.google.com/vpc/docs/configure-private-service-connection#pricing)
- [Cloud Load Balancing Pricing](https://cloud.google.com/load-balancing/pricing)
- [Cloud CDN Pricing](https://cloud.google.com/cdn/pricing)
- [Infracost](https://www.infracost.io/)
- [GCP Cost Optimizer](https://cloud.google.com/cost-management)

## Support

For cost-related questions:

1. Review [Cloud Billing Documentation](https://cloud.google.com/billing/docs)
2. Check [Cost Analysis in Cloud Console](https://console.cloud.google.com/billing)
3. Use [Resource Optimizer](https://cloud.google.com/resource-manager/docs/managing-resources/optimize-resources)
4. Contact GCP Sales for volume discounts
