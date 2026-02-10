# Artifact Publishing Pattern

## Overview

Artifact publishing allows publishing different types of artifacts (emails, calendar events, documents) to Google Workspace services through a single unified interface.

## Architecture

```
domain/artifact.rs
  ↓ (pure types, no external deps)
port/artifact_publisher.rs
  ↓ (trait definition)
adapter/workspace_publisher.rs
  ↓ (Google APIs implementation)
```

## Domain Types

All domain types in `domain/artifact.rs`:

```rust
// Main artifact enum - routes to correct service
pub enum Artifact {
    Email(EmailArtifact),
    CalendarEvent(CalendarArtifact),
    Document(DriveArtifact),
}

// Email-specific artifact
pub struct EmailArtifact {
    pub id: Uuid,
    pub to: String,          // Primary recipient
    pub cc: Vec<String>,     // Carbon copy
    pub bcc: Vec<String>,    // Blind carbon copy
    pub subject: String,
    pub body: String,        // Plain text body
    pub html_body: Option<String>,  // Optional HTML
    pub attachments: Vec<String>,   // Filenames only
    pub labels: Vec<String>, // Gmail labels
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

// Calendar event artifact
pub struct CalendarArtifact {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub attendees: Vec<String>,      // Email addresses
    pub location: Option<String>,
    pub timezone: String,
    pub is_all_day: bool,
    pub reminders: Vec<u32>,         // Minutes before event
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

// Document artifact (Drive)
pub struct DriveArtifact {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub content: Vec<u8>,           // Raw binary content
    pub parent_folder_id: Option<String>,
    pub sharing_permissions: Vec<SharingPermission>,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

// Result of publishing
pub struct PublishResult {
    pub artifact_id: Uuid,
    pub external_id: String,        // Service-assigned ID
    pub artifact_type: String,
    pub published_at: DateTime<Utc>,
    pub response_metadata: HashMap<String, serde_json::Value>,
}

// Comprehensive error type
#[derive(Debug, Clone, thiserror::Error)]
pub enum ArtifactPublishError {
    #[error("Authentication failed: {reason}")]
    AuthenticationError { reason: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid artifact data: {reason}")]
    InvalidData { reason: String },

    #[error("API request failed: {reason}")]
    RequestFailed { reason: String },

    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },

    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    #[error("Unsupported artifact type: {artifact_type}")]
    UnsupportedArtifactType { artifact_type: String },
}
```

## Port Trait

In `port/artifact_publisher.rs`:

```rust
#[async_trait]
pub trait ArtifactPublisher: Send + Sync {
    /// Generic publish - routes based on artifact type
    async fn publish(&self, artifact: &Artifact)
        -> Result<PublishResult, ArtifactPublishError>;

    /// Batch publishing (default: serial)
    async fn publish_batch(&self, artifacts: &[Artifact])
        -> Result<Vec<PublishResult>, ArtifactPublishError>;

    /// Gmail-specific: send email
    async fn send_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        html_body: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Calendar-specific: create event
    async fn create_calendar_event(
        &self,
        title: &str,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        description: Option<&str>,
        location: Option<&str>,
        attendees: &[String],
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Drive-specific: upload document
    async fn upload_document(
        &self,
        filename: &str,
        mime_type: &str,
        content: &[u8],
        parent_folder_id: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Validation without publishing (dry-run)
    fn validate_artifact(&self, artifact: &Artifact)
        -> Result<(), ArtifactPublishError>;

    /// Check if authenticated
    async fn check_authentication(&self)
        -> Result<(), ArtifactPublishError>;

    /// Account identification
    fn account_name(&self) -> &str;
}
```

## Adapter Implementation

In `adapter/workspace_publisher.rs`:

### Configuration

```rust
pub struct WorkspacePublisherConfig {
    pub access_token: String,
    pub account_email: String,
    pub refresh_token: Option<String>,
    pub default_labels: Vec<String>,      // Gmail labels
    pub calendar_id: Option<String>,      // Default calendar
    pub default_folder_id: Option<String>, // Default Drive folder
}
```

### Implementation Strategy

1. **Validation First**: All methods validate inputs before API calls
2. **Email Validation**: Check format, required fields
3. **Time Validation**: Ensure start < end for calendar events
4. **Content Validation**: Ensure documents not empty
5. **Response Metadata**: Capture service-specific response data

### Simulation Methods (Ready for Real APIs)

```rust
impl GoogleWorkspacePublisher {
    async fn simulate_gmail_api_call(...) -> Result<PublishResult, ...> {
        // Real implementation would:
        // 1. Build MIME message
        // 2. Encode base64url
        // 3. Call gmail API: users().messages().send()
        // 4. Extract message ID from response

        let message_id = format!("msg_{}", Uuid::new_v4().to_string()[..8]);
        Ok(PublishResult {
            artifact_id: Uuid::new_v4(),
            external_id: message_id,
            artifact_type: "email".to_string(),
            published_at: Utc::now(),
            response_metadata: metadata,
        })
    }

    async fn simulate_calendar_api_call(...) -> Result<PublishResult, ...> {
        // Real implementation would:
        // 1. Build Calendar Event object
        // 2. Call calendar API: events().insert()
        // 3. Extract event ID from response

        let event_id = format!("evt_{}", Uuid::new_v4().to_string()[..12]);
        // ...
    }

    async fn simulate_drive_api_call(...) -> Result<PublishResult, ...> {
        // Real implementation would:
        // 1. Build File metadata object
        // 2. Call drive API: files().create() with media upload
        // 3. Extract file ID from response

        let file_id = format!("file_{}", Uuid::new_v4().to_string()[..16]);
        // ...
    }
}
```

## Builder Pattern

Convenience constructors for creating artifacts:

```rust
// Email
let email = Artifact::email("user@example.com", "Subject", "Body");

// Calendar event
let start = Utc::now();
let end = start + Duration::hours(1);
let event = Artifact::calendar_event("Meeting", start, end);

// Document
let doc = Artifact::document("report.pdf", "application/pdf", vec![...]);

// Mutating builders
let mut event = Artifact::calendar_event("Meeting", start, end);
event.add_attendee("attendee@example.com".to_string());
event.set_description("Team sync meeting".to_string());
event.set_location("Conference room A".to_string());
event.add_reminder(15);  // 15 minutes before
```

## Feature Gating

In `Cargo.toml`:

```toml
[dependencies]
google-gmail1 = { version = "7.0", optional = true }
google-calendar3 = { version = "7.0", optional = true }
google-drive3 = { version = "7.0", optional = true }
hyper = { version = "1.0", optional = true }
hyper-rustls = { version = "0.27", optional = true }
yup-oauth2 = { version = "11.0", optional = true }

[features]
workspace-publisher = [
    "google-gmail1",
    "google-calendar3",
    "google-drive3",
    "hyper",
    "hyper-rustls",
    "yup-oauth2",
]
```

Module exports with feature gate:

```rust
#[cfg(feature = "workspace-publisher")]
pub mod workspace_publisher;

#[cfg(feature = "workspace-publisher")]
pub use workspace_publisher::{GoogleWorkspacePublisher, WorkspacePublisherConfig};
```

## Testing

40+ tests covering:

- **Domain tests**:
  - Artifact creation
  - Validation of missing fields
  - Validation of invalid data
  - Builder pattern usage
  - Serialization/deserialization

- **Adapter tests**:
  - Email sending
  - Email validation
  - Calendar event creation
  - Calendar validation (time ordering)
  - Document upload
  - Document validation
  - Batch publishing
  - Authentication checking
  - Response metadata

Example test:

```rust
#[tokio::test]
async fn test_send_email() {
    let config = WorkspacePublisherConfig::new("token", "user@example.com");
    let publisher = GoogleWorkspacePublisher::new(config);

    let result = publisher
        .send_email(
            "recipient@example.com",
            "Test Subject",
            "Test body",
            None,
        )
        .await;

    assert!(result.is_ok());
    let publish_result = result.unwrap();
    assert_eq!(publish_result.artifact_type, "email");
    assert!(!publish_result.external_id.is_empty());
}
```

## Integration Path to Real APIs

To integrate real Google APIs:

1. **Replace simulation methods**:
   ```rust
   async fn send_email_real(
       access_token: &str,
       to: &str,
       subject: &str,
       body: &str,
   ) -> Result<PublishResult, ArtifactPublishError> {
       // Use google_gmail1 crate
       let client = gmail1::Gmail::new(...);
       let msg = // build Message
       let result = client.users().messages_send(msg, "me").await?;
       Ok(PublishResult { ... })
   }
   ```

2. **Add OAuth2 flow**:
   ```rust
   use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

   let auth = ServiceAccountAuthenticator::builder(key)
       .build()
       .await?;
   ```

3. **Error mapping**:
   ```rust
   impl From<google_gmail1::Error> for ArtifactPublishError {
       fn from(e: google_gmail1::Error) -> Self {
           ArtifactPublishError::RequestFailed { reason: e.to_string() }
       }
   }
   ```

4. **Implement streaming uploads** for large documents via Drive resumable upload API

## Key Design Decisions

1. **Single Artifact enum**: Unified interface despite different services
2. **Validation before API calls**: Fail fast on client errors
3. **Response metadata**: Capture service-specific data for auditing
4. **Feature-gated**: Optional compilation, doesn't bloat workspace
5. **Async-first**: All operations are async (no blocking calls)
6. **Error specificity**: Different error types for different failure modes
7. **Simulation ready**: Adapter prepared for real API without refactoring

## Common Patterns

### Validation Pattern
```rust
impl EmailArtifact {
    pub fn validate(&self) -> ArtifactResult<()> {
        if self.to.is_empty() {
            return Err(ArtifactPublishError::MissingField { ... });
        }
        if !self.to.contains('@') {
            return Err(ArtifactPublishError::InvalidData { ... });
        }
        Ok(())
    }
}
```

### Builder Pattern
```rust
let mut artifact = Artifact::calendar_event("Title", start, end);
if let Artifact::CalendarEvent(event) = &mut artifact {
    event.add_attendee("user@example.com".to_string());
}
```

### Error Handling
```rust
match publisher.publish(&artifact).await {
    Ok(result) => println!("Published: {}", result.external_id),
    Err(ArtifactPublishError::AuthenticationError { reason }) => eprintln!("Auth failed: {}", reason),
    Err(ArtifactPublishError::MissingField { field }) => eprintln!("Missing: {}", field),
    Err(e) => eprintln!("Error: {}", e),
}
```
