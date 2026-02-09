//! Authentication gate demonstration
//!
//! This example shows how to use the various auth gate implementations:
//! - JWT authentication with HMAC secret
//! - JWT authentication with RSA keys
//! - Google Workspace OAuth2 validation
//! - Service account validation
//! - Composite auth gate combining multiple validators

use osiris_edge::adapter::auth_gate::{
    CompositeAuthGate, GoogleWorkspaceAuthGate, JwtAuthGate, ServiceAccountAuthGate,
};
use osiris_edge::domain::{AuthRequest, PrincipalType, TokenValidationConfig};
use osiris_edge::port::auth_gate::AuthGate;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Auth Gate Demo ===\n");

    // Example 1: JWT with HMAC secret
    demo_jwt_hmac().await?;

    // Example 2: Composite auth gate
    demo_composite_auth().await?;

    // Example 3: Service account validation
    demo_service_account().await?;

    Ok(())
}

async fn demo_jwt_hmac() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Demo 1: JWT with HMAC Secret ---");

    let secret = b"my-secret-key-at-least-32-bytes-long";
    let config = TokenValidationConfig::new()
        .with_issuer("https://auth.example.com".to_string())
        .with_audience("https://api.example.com".to_string());

    let auth_gate = JwtAuthGate::new_with_secret(secret).with_config(config);

    // In a real app, you'd receive this token from the client
    // For demo purposes, we'll show how to use the auth gate
    println!("Created JWT auth gate with HMAC-SHA256");
    println!("Expected issuer: https://auth.example.com");
    println!("Expected audience: https://api.example.com\n");

    // Example: validate a token (would fail without a real token)
    let demo_request = AuthRequest::new("eyJ...fake-token...".to_string());
    match auth_gate.authenticate(&demo_request).await {
        Ok(principal) => {
            println!("Authenticated: {:?}", principal);
        }
        Err(e) => {
            println!("Expected error (demo token): {}\n", e);
        }
    }

    Ok(())
}

async fn demo_composite_auth() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Demo 2: Composite Auth Gate ---");

    let secret = b"my-secret-key-at-least-32-bytes-long";

    // Build JWT validator
    let jwt_validator = JwtAuthGate::new_with_secret(secret);

    // Build Google Workspace validator
    let google_validator = GoogleWorkspaceAuthGate::new()
        .with_client_id("123456789.apps.googleusercontent.com".to_string())
        .with_required_scopes(vec![
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
            "https://www.googleapis.com/auth/userinfo.profile".to_string(),
        ]);

    // Build service account validator
    let service_account_validator = ServiceAccountAuthGate::new_with_secret(secret)
        .with_allowed_service_account("backend-service@project.iam.gserviceaccount.com".to_string())
        .with_permissions(
            "backend-service@project.iam.gserviceaccount.com".to_string(),
            vec!["read".to_string(), "write".to_string()],
        );

    // Composite auth gate tries validators in order
    let composite_gate = CompositeAuthGate::builder()
        .with_jwt_validator(jwt_validator)
        .with_google_validator(google_validator)
        .with_service_account_validator(service_account_validator)
        .build();

    println!("Created composite auth gate with:");
    println!("  - JWT validator (HMAC-SHA256)");
    println!("  - Google Workspace OAuth2 validator");
    println!("  - Service account validator");
    println!();
    println!("The composite gate tries each validator in sequence");
    println!("until one successfully authenticates the token.\n");

    Ok(())
}

async fn demo_service_account() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Demo 3: Service Account Validation ---");

    let secret = b"service-account-secret-key-32-bytes";

    let service_account_gate = ServiceAccountAuthGate::new_with_secret(secret)
        .with_allowed_service_account("api-gateway@project.iam.gserviceaccount.com".to_string())
        .with_allowed_service_account("worker-service@project.iam.gserviceaccount.com".to_string())
        .with_permissions(
            "api-gateway@project.iam.gserviceaccount.com".to_string(),
            vec!["gateway.route".to_string(), "gateway.transform".to_string()],
        )
        .with_permissions(
            "worker-service@project.iam.gserviceaccount.com".to_string(),
            vec!["worker.execute".to_string(), "worker.report".to_string()],
        );

    println!("Created service account auth gate");
    println!("Allowed service accounts:");
    println!("  - api-gateway@project.iam.gserviceaccount.com");
    println!("      Permissions: gateway.route, gateway.transform");
    println!("  - worker-service@project.iam.gserviceaccount.com");
    println!("      Permissions: worker.execute, worker.report");
    println!();
    println!("Service account tokens are validated as JWTs with additional checks");
    println!("for principal type and permission management.\n");

    Ok(())
}
