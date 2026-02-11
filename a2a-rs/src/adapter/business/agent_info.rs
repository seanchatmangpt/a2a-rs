//! A simple agent info provider implementation

// This module is already conditionally compiled with #[cfg(feature = "server")] in mod.rs

use async_trait::async_trait;
use std::collections::HashMap;

use crate::{
    domain::{A2AError, AgentCapabilities, AgentCard, AgentProvider, AgentSkill, SecurityScheme},
    services::server::AgentInfoProvider,
};

/// A simple agent info provider that returns a fixed agent card
#[derive(Clone)]
pub struct SimpleAgentInfo {
    /// The agent card to return
    card: AgentCard,
}

impl SimpleAgentInfo {
    /// Create a new agent info provider with the given name and URL
    pub fn new(name: String, url: String) -> Self {
        Self {
            card: AgentCard {
                name,
                description: "Agent description".to_string(),
                url,
                provider: None,
                version: "1.0.0".to_string(),
                protocol_version: "0.3.0".to_string(),
                preferred_transport: "JSONRPC".to_string(),
                additional_interfaces: None,
                icon_url: None,
                documentation_url: None,
                capabilities: AgentCapabilities::default(),
                security_schemes: None,
                security: None,
                default_input_modes: vec!["text".to_string()],
                default_output_modes: vec!["text".to_string()],
                skills: Vec::new(),
                signatures: None,
                supports_authenticated_extended_card: None,
            },
        }
    }

    /// Set the description of the agent
    pub fn with_description(mut self, description: String) -> Self {
        self.card.description = description;
        self
    }

    /// Set the provider of the agent
    pub fn with_provider(mut self, organization: String, url: String) -> Self {
        self.card.provider = Some(AgentProvider { organization, url });
        self
    }

    /// Set the version of the agent
    pub fn with_version(mut self, version: String) -> Self {
        self.card.version = version;
        self
    }

    /// Set the documentation URL of the agent
    pub fn with_documentation_url(mut self, url: String) -> Self {
        self.card.documentation_url = Some(url);
        self
    }

    /// Enable streaming capability
    pub fn with_streaming(mut self) -> Self {
        self.card.capabilities.streaming = true;
        self
    }

    /// Enable push notifications capability
    pub fn with_push_notifications(mut self) -> Self {
        self.card.capabilities.push_notifications = true;
        self
    }

    /// Enable state transition history capability
    pub fn with_state_transition_history(mut self) -> Self {
        self.card.capabilities.state_transition_history = true;
        self
    }

    /// Enable authenticated extended card support (v0.3.0)
    pub fn with_authenticated_extended_card(mut self) -> Self {
        self.card.supports_authenticated_extended_card = Some(true);
        self
    }

    /// Set the authentication schemes
    ///
    /// Takes a vector of tuples where each tuple contains a scheme name and the SecurityScheme.
    /// The schemes are stored in the AgentCard's security_schemes field as a HashMap.
    ///
    /// # Example
    /// ```rust
    /// use a2a_rs::{SecurityScheme, SimpleAgentInfo};
    /// use std::collections::HashMap;
    ///
    /// let schemes = vec![
    ///     ("bearer".to_string(), SecurityScheme::Http {
    ///         scheme: "bearer".to_string(),
    ///         bearer_format: Some("JWT".to_string()),
    ///         description: Some("Bearer token authentication".to_string()),
    ///     }),
    /// ];
    ///
    /// let agent = SimpleAgentInfo::new("MyAgent".to_string(), "https://example.com".to_string())
    ///     .with_authentication(schemes);
    /// ```
    pub fn with_authentication(mut self, schemes: Vec<(String, SecurityScheme)>) -> Self {
        if !schemes.is_empty() {
            self.card.security_schemes = Some(schemes.into_iter().collect());
        }
        self
    }

    /// Add a single security scheme to the agent card
    ///
    /// This method allows adding authentication schemes one at a time using the builder pattern.
    /// If a security scheme with the same name already exists, it will be replaced.
    ///
    /// # Example
    /// ```rust
    /// use a2a_rs::{SecurityScheme, SimpleAgentInfo};
    ///
    /// let agent = SimpleAgentInfo::new("MyAgent".to_string(), "https://example.com".to_string())
    ///     .add_security_scheme(
    ///         "bearer".to_string(),
    ///         SecurityScheme::Http {
    ///             scheme: "bearer".to_string(),
    ///             bearer_format: Some("JWT".to_string()),
    ///             description: Some("Bearer token authentication".to_string()),
    ///         }
    ///     )
    ///     .add_security_scheme(
    ///         "mtls".to_string(),
    ///         SecurityScheme::MutualTls {
    ///             description: Some("Client certificate authentication".to_string()),
    ///         }
    ///     );
    /// ```
    pub fn add_security_scheme(mut self, name: String, scheme: SecurityScheme) -> Self {
        if self.card.security_schemes.is_none() {
            self.card.security_schemes = Some(HashMap::new());
        }
        self.card.security_schemes.as_mut().unwrap().insert(name, scheme);
        self
    }

    /// Set the default security requirements for the agent
    ///
    /// This sets the top-level `security` field which specifies which authentication
    /// schemes and scopes are required by default. Each requirement is a HashMap mapping
    /// scheme names to required scopes.
    ///
    /// # Example
    /// ```rust
    /// use a2a_rs::{SimpleAgentInfo};
    /// use std::collections::HashMap;
    ///
    /// let mut security_req = HashMap::new();
    /// security_req.insert("bearer".to_string(), vec!["read:tasks".to_string(), "write:tasks".to_string()]);
    ///
    /// let agent = SimpleAgentInfo::new("MyAgent".to_string(), "https://example.com".to_string())
    ///     .with_security_requirements(vec![security_req]);
    /// ```
    pub fn with_security_requirements(mut self, requirements: Vec<HashMap<String, Vec<String>>>) -> Self {
        if !requirements.is_empty() {
            self.card.security = Some(requirements);
        }
        self
    }

    /// Add an input mode
    pub fn add_input_mode(mut self, mode: String) -> Self {
        self.card.default_input_modes.push(mode);
        self
    }

    /// Add an output mode
    pub fn add_output_mode(mut self, mode: String) -> Self {
        self.card.default_output_modes.push(mode);
        self
    }

    /// Add a basic skill
    pub fn add_skill(mut self, id: String, name: String, description: Option<String>) -> Self {
        let skill = AgentSkill::new(
            id,
            name,
            description.unwrap_or_else(|| "Skill description".to_string()),
            Vec::new(), // empty tags
        );

        self.card.skills.push(skill);
        self
    }

    /// Add a comprehensive skill with all details
    #[allow(clippy::too_many_arguments)]
    pub fn add_comprehensive_skill(
        mut self,
        id: String,
        name: String,
        description: Option<String>,
        tags: Option<Vec<String>>,
        examples: Option<Vec<String>>,
        input_modes: Option<Vec<String>>,
        output_modes: Option<Vec<String>>,
    ) -> Self {
        let skill = AgentSkill::comprehensive(
            id,
            name,
            description.unwrap_or_else(|| "Skill description".to_string()),
            tags.unwrap_or_default(),
            examples,
            input_modes,
            output_modes,
            None, // security - v0.3.0
        );

        self.card.skills.push(skill);
        self
    }

    /// Add a skill using the AgentSkill builder
    pub fn add_skill_object(mut self, skill: AgentSkill) -> Self {
        self.card.skills.push(skill);
        self
    }

    /// Replace all skills with a new set
    pub fn with_skills(mut self, skills: Vec<AgentSkill>) -> Self {
        self.card.skills = skills;
        self
    }

    /// Get all currently defined skills
    pub fn get_skills(&self) -> &Vec<AgentSkill> {
        &self.card.skills
    }

    /// Get a skill by ID
    pub fn get_skill_by_id(&self, id: &str) -> Option<&AgentSkill> {
        self.card.skills.iter().find(|skill| skill.id == id)
    }

    /// Add a new skill or update an existing one
    pub fn add_or_update_skill(&mut self, skill: AgentSkill) -> &mut Self {
        // Check if the skill with this ID already exists
        if let Some(index) = self.card.skills.iter().position(|s| s.id == skill.id) {
            // Update the existing skill
            self.card.skills[index] = skill;
        } else {
            // Add a new skill
            self.card.skills.push(skill);
        }
        self
    }

    /// Remove a skill by ID
    pub fn remove_skill(&mut self, id: &str) -> bool {
        let len_before = self.card.skills.len();
        self.card.skills.retain(|skill| skill.id != id);
        self.card.skills.len() < len_before
    }

    /// Update a skill's details
    #[allow(clippy::too_many_arguments)]
    pub fn update_skill(
        &mut self,
        id: &str,
        name: Option<String>,
        description: Option<Option<String>>,
        tags: Option<Option<Vec<String>>>,
        examples: Option<Option<Vec<String>>>,
        input_modes: Option<Option<Vec<String>>>,
        output_modes: Option<Option<Vec<String>>>,
    ) -> bool {
        if let Some(skill) = self.card.skills.iter_mut().find(|s| s.id == id) {
            if let Some(name_val) = name {
                skill.name = name_val;
            }

            if let Some(desc) = description {
                skill.description = desc.unwrap_or_else(|| "Updated description".to_string());
            }

            if let Some(tags_val) = tags {
                skill.tags = tags_val.unwrap_or_default();
            }

            if let Some(examples_val) = examples {
                skill.examples = examples_val;
            }

            if let Some(input_modes_val) = input_modes {
                skill.input_modes = input_modes_val;
            }

            if let Some(output_modes_val) = output_modes {
                skill.output_modes = output_modes_val;
            }

            true
        } else {
            false
        }
    }
}

#[async_trait]
impl AgentInfoProvider for SimpleAgentInfo {
    async fn get_agent_card(&self) -> Result<AgentCard, A2AError> {
        Ok(self.card.clone())
    }

    // Override the default implementation for better performance
    async fn get_skills(&self) -> Result<Vec<AgentSkill>, A2AError> {
        Ok(self.card.skills.clone())
    }

    // Override the default implementation for better performance
    async fn get_skill_by_id(&self, id: &str) -> Result<Option<AgentSkill>, A2AError> {
        Ok(self
            .card
            .skills
            .iter()
            .find(|skill| skill.id == id)
            .cloned())
    }

    // Override the default implementation for better performance
    async fn has_skill(&self, id: &str) -> Result<bool, A2AError> {
        Ok(self.card.skills.iter().any(|skill| skill.id == id))
    }

    // Override to provide authenticated extended card when configured (v0.3.0)
    async fn get_authenticated_extended_card(&self) -> Result<AgentCard, A2AError> {
        if self
            .card
            .supports_authenticated_extended_card
            .unwrap_or(false)
        {
            // Return the same card for now
            // In a real implementation, this might include additional authenticated-only fields
            Ok(self.card.clone())
        } else {
            Err(A2AError::AuthenticatedExtendedCardNotConfigured)
        }
    }
}
