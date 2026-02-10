//! Google Cloud Pub/Sub adapter for consuming entitlement events.

#[cfg(feature = "pubsub")]
use crate::domain::{EntitlementEvent, PubSubMessage};
#[cfg(feature = "pubsub")]
use crate::port::{EventConsumer, EventConsumerError, EventConsumerResult};
#[cfg(feature = "pubsub")]
use async_trait::async_trait;
#[cfg(feature = "pubsub")]
use base64::Engine;
#[cfg(feature = "pubsub")]
use google_cloud_pubsub::client::{Client, ClientConfig};
#[cfg(feature = "pubsub")]
use google_cloud_pubsub::subscription::Subscription;
#[cfg(feature = "pubsub")]
use tracing::{debug, error, info, warn};

/// Google Cloud Pub/Sub consumer adapter
#[cfg(feature = "pubsub")]
#[derive(Clone)]
pub struct PubSubConsumer {
    subscription: Subscription,
    #[allow(dead_code)]
    project_id: String,
    subscription_id: String,
}

#[cfg(feature = "pubsub")]
impl PubSubConsumer {
    /// Create a new Pub/Sub consumer
    ///
    /// # Arguments
    ///
    /// * `project_id` - Google Cloud project ID
    /// * `subscription_id` - Pub/Sub subscription ID for the entitlement events topic
    ///
    /// # Returns
    ///
    /// A new PubSubConsumer instance
    ///
    /// # Errors
    ///
    /// Returns an error if the client cannot be initialized or the subscription cannot be found
    pub async fn new(project_id: String, subscription_id: String) -> EventConsumerResult<Self> {
        info!(
            "Initializing Pub/Sub consumer for project {} subscription {}",
            project_id, subscription_id
        );

        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| EventConsumerError::ConnectionError(e.to_string()))?;

        let client = Client::new(config)
            .await
            .map_err(|e| EventConsumerError::ConnectionError(e.to_string()))?;

        let subscription = client.subscription(&subscription_id);

        Ok(Self {
            subscription,
            project_id,
            subscription_id,
        })
    }
}

#[cfg(feature = "pubsub")]
#[async_trait]
impl EventConsumer for PubSubConsumer {
    async fn consume<F, Fut>(&self, handler: F) -> EventConsumerResult<()>
    where
        F: Fn(EntitlementEvent) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        info!(
            "Starting event consumption from subscription {}",
            self.subscription_id
        );

        loop {
            // Pull messages in batches
            match self.pull_messages(10).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        debug!("No messages received, waiting...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }

                    info!("Received {} messages", messages.len());

                    for message in messages {
                        match self.parse_event(&message) {
                            Ok(event) => {
                                debug!("Parsed event: {:?}", event);

                                match handler(event).await {
                                    Ok(()) => {
                                        if let Err(e) = self.acknowledge(&message.message_id).await
                                        {
                                            error!(
                                                "Failed to acknowledge message {}: {}",
                                                message.message_id, e
                                            );
                                        } else {
                                            debug!("Acknowledged message {}", message.message_id);
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Handler failed for message {}: {}",
                                            message.message_id, e
                                        );
                                        // Don't acknowledge failed messages - they'll be redelivered
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message {}: {}", message.message_id, e);
                                // Acknowledge unparseable messages to avoid infinite retry
                                let _ = self.acknowledge(&message.message_id).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to pull messages: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn pull_messages(&self, max_messages: i32) -> EventConsumerResult<Vec<PubSubMessage>> {
        let messages = self
            .subscription
            .pull(max_messages, None)
            .await
            .map_err(|e| EventConsumerError::SubscriptionError(e.to_string()))?;

        Ok(messages
            .into_iter()
            .map(|msg| {
                let ack_id = msg.ack_id().to_string();
                PubSubMessage {
                    data: base64::engine::general_purpose::STANDARD.encode(&msg.message.data),
                    attributes: msg.message.attributes,
                    message_id: ack_id,
                    publish_time: chrono::Utc::now(), // Note: actual publish_time from msg.message.publish_time
                }
            })
            .collect())
    }

    fn parse_event(&self, message: &PubSubMessage) -> EventConsumerResult<EntitlementEvent> {
        // Decode base64 data
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&message.data)
            .map_err(|e| EventConsumerError::DecodeError(e.to_string()))?;

        // Parse JSON payload
        let event: EntitlementEvent = serde_json::from_slice(&decoded)
            .map_err(|e| EventConsumerError::ParseError(e.to_string()))?;

        Ok(event)
    }

    async fn acknowledge(&self, message_id: &str) -> EventConsumerResult<()> {
        self.subscription
            .ack(vec![message_id.to_string()])
            .await
            .map_err(|e| EventConsumerError::AckError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(all(test, feature = "pubsub"))]
mod tests {
    use super::*;
    use crate::domain::EntitlementEventType;

    #[test]
    fn test_parse_event() {
        // This would require a mock subscription, skipping for now
        // Real integration tests should use emulator or test project
    }
}
