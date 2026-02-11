//! OpenAPI specification generation for the A2A protocol server
//!
//! Generates OpenAPI 3.0 specifications from the A2A protocol definition,
//! including all JSON-RPC methods, authentication schemes, and data models.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// OpenAPI specification builder for A2A protocol
#[derive(Debug, Clone)]
pub struct OpenApiBuilder {
    /// Service info
    info: OpenApiInfo,

    /// Server configuration
    servers: Vec<OpenApiServer>,

    /// Authentication schemes
    security_schemes: Vec<OpenApiSecurityScheme>,

    /// Include health check endpoints
    include_health: bool,

    /// Include spec endpoint
    include_spec_endpoint: bool,
}

/// OpenAPI service information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<OpenApiContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<OpenApiLicense>,
}

/// Contact information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiContact {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// License information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiLicense {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiServer {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
}

/// Security scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum OpenApiSecurityScheme {
    #[serde(rename = "http")]
    Http {
        scheme: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bearer_format: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    #[serde(rename = "apiKey")]
    ApiKey {
        name: String,
        in_: String, // "header", "query", or "cookie"
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    #[serde(rename = "oauth2")]
    OAuth2 {
        flows: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    #[serde(rename = "openIdConnect")]
    OpenIdConnect {
        open_id_connect_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

impl Default for OpenApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenApiBuilder {
    /// Create a new OpenAPI builder with defaults
    pub fn new() -> Self {
        Self {
            info: OpenApiInfo {
                title: "A2A Protocol Server".to_string(),
                version: "0.3.0".to_string(),
                description: Some(
                    "Agent-to-Agent (A2A) Protocol v0.3.0 Server Implementation".to_string(),
                ),
                terms_of_service: None,
                contact: None,
                license: Some(OpenApiLicense {
                    name: "MIT".to_string(),
                    url: Some("https://opensource.org/licenses/MIT".to_string()),
                }),
            },
            servers: vec![],
            security_schemes: vec![],
            include_health: true,
            include_spec_endpoint: true,
        }
    }

    /// Set service title
    pub fn with_title(mut self, title: String) -> Self {
        self.info.title = title;
        self
    }

    /// Set service version
    pub fn with_version(mut self, version: String) -> Self {
        self.info.version = version;
        self
    }

    /// Set service description
    pub fn with_description(mut self, description: String) -> Self {
        self.info.description = Some(description);
        self
    }

    /// Set contact information
    pub fn with_contact(mut self, contact: OpenApiContact) -> Self {
        self.info.contact = Some(contact);
        self
    }

    /// Add a server
    pub fn add_server(mut self, url: String, description: Option<String>) -> Self {
        self.servers.push(OpenApiServer {
            url,
            description,
            variables: None,
        });
        self
    }

    /// Add HTTP security scheme (bearer token, basic, etc.)
    pub fn add_http_security(mut self, scheme: String, description: Option<String>) -> Self {
        self.security_schemes.push(OpenApiSecurityScheme::Http {
            scheme: scheme.clone(),
            bearer_format: if scheme == "bearer" {
                Some("JWT".to_string())
            } else {
                None
            },
            description,
        });
        self
    }

    /// Add API key security scheme
    pub fn add_api_key_security(
        mut self,
        name: String,
        location: String,
        description: Option<String>,
    ) -> Self {
        self.security_schemes.push(OpenApiSecurityScheme::ApiKey {
            name,
            in_: location,
            description,
        });
        self
    }

    /// Include health check endpoints
    pub fn include_health(mut self, include: bool) -> Self {
        self.include_health = include;
        self
    }

    /// Include spec endpoint itself
    pub fn include_spec_endpoint(mut self, include: bool) -> Self {
        self.include_spec_endpoint = include;
        self
    }

    /// Build the OpenAPI specification
    pub fn build(&self) -> Value {
        let mut paths = json!({});

        // Main JSON-RPC endpoint
        paths["/"] = json!({
            "post": {
                "summary": "JSON-RPC endpoint",
                "description": "Main endpoint for all JSON-RPC 2.0 requests",
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {
                                "oneOf": [
                                    {"$ref": "#/components/schemas/JSONRPCRequest"},
                                    {"$ref": "#/components/schemas/JSONRPCBatchRequest"}
                                ]
                            }
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Successful response",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "oneOf": [
                                        {"$ref": "#/components/schemas/JSONRPCResponse"},
                                        {"$ref": "#/components/schemas/JSONRPCBatchResponse"}
                                    ]
                                }
                            }
                        }
                    },
                    "400": {
                        "description": "Invalid request",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/JSONRPCError"}
                            }
                        }
                    }
                },
                "x-rate-limit": "100/minute"
            }
        });

        // Agent card endpoint (RFC 8615 well-known URI)
        paths["/.well-known/agent-card.json"] = json!({
            "get": {
                "summary": "Get agent card",
                "description": "Returns the agent's card describing capabilities, skills, and metadata",
                "responses": {
                    "200": {
                        "description": "Agent card",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/AgentCard"}
                            }
                        }
                    }
                }
            }
        });

        // Skills endpoints
        paths["/skills"] = json!({
            "get": {
                "summary": "Get all skills",
                "description": "Returns a list of all available agent skills",
                "responses": {
                    "200": {
                        "description": "List of skills",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": {"$ref": "#/components/schemas/AgentSkill"}
                                }
                            }
                        }
                    }
                }
            }
        });

        paths["/skills/{id}"] = json!({
            "get": {
                "summary": "Get skill by ID",
                "description": "Returns a specific skill by its ID",
                "parameters": [
                    {
                        "name": "id",
                        "in": "path",
                        "required": true,
                        "schema": {"type": "string"}
                    }
                ],
                "responses": {
                    "200": {
                        "description": "Skill details",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/AgentSkill"}
                            }
                        }
                    },
                    "404": {
                        "description": "Skill not found"
                    }
                }
            }
        });

        // Health check endpoints
        if self.include_health {
            paths["/health"] = json!({
                "get": {
                    "summary": "Health check",
                    "description": "Returns the health status of the service and its components",
                    "responses": {
                        "200": {
                            "description": "Health status",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/HealthCheckResponse"}
                                }
                            }
                        },
                        "503": {
                            "description": "Service unhealthy"
                        }
                    }
                }
            });

            paths["/ready"] = json!({
                "get": {
                    "summary": "Readiness check",
                    "description": "Returns whether the service is ready to accept requests",
                    "responses": {
                        "200": {
                            "description": "Service is ready",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ReadinessCheckResponse"}
                                }
                            }
                        },
                        "503": {
                            "description": "Service not ready"
                        }
                    }
                }
            });

            paths["/live"] = json!({
                "get": {
                    "summary": "Liveness check",
                    "description": "Returns whether the service is alive",
                    "responses": {
                        "200": {
                            "description": "Service is alive",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/LivenessCheckResponse"}
                                }
                            }
                        }
                    }
                }
            });
        }

        // OpenAPI spec endpoint
        if self.include_spec_endpoint {
            paths["/openapi.json"] = json!({
                "get": {
                    "summary": "OpenAPI specification",
                    "description": "Returns the OpenAPI 3.0 specification for this service",
                    "responses": {
                        "200": {
                            "description": "OpenAPI specification",
                            "content": {
                                "application/json": {
                                    "schema": {"type": "object"}
                                }
                            }
                        }
                    }
                }
            });
        }

        // Build security schemes
        let mut security_schemes = json!({});
        let mut security_requirements = vec![];

        for (idx, scheme) in self.security_schemes.iter().enumerate() {
            let name = format!("scheme_{}", idx);
            security_schemes[&name] = serde_json::to_value(scheme).unwrap();
            let mut requirement = serde_json::Map::new();
            requirement.insert(name, json!([]));
            security_requirements.push(json!(requirement));
        }

        // If no auth schemes, add empty security (no auth required)
        if self.security_schemes.is_empty() {
            security_requirements.push(json!( {}));
        }

        json!({
            "openapi": "3.0.0",
            "info": self.info,
            "servers": self.servers,
            "paths": paths,
            "components": {
                "securitySchemes": security_schemes,
                "schemas": self.build_schemas()
            },
            "security": security_requirements
        })
    }

    /// Build JSON schemas for A2A types
    fn build_schemas(&self) -> Value {
        json!({
            "JSONRPCRequest": {
                "type": "object",
                "required": ["jsonrpc", "method", "id"],
                "properties": {
                    "jsonrpc": {"type": "string", "enum": ["2.0"]},
                    "method": {"type": "string"},
                    "params": {
                        "oneOf": [
                            {"type": "object"},
                            {"type": "array"}
                        ]
                    },
                    "id": {
                        "oneOf": [
                            {"type": "string"},
                            {"type": "number"},
                            {"type": "null"}
                        ]
                    }
                }
            },
            "JSONRPCBatchRequest": {
                "type": "array",
                "items": {"$ref": "#/components/schemas/JSONRPCRequest"}
            },
            "JSONRPCResponse": {
                "type": "object",
                "required": ["jsonrpc", "id"],
                "properties": {
                    "jsonrpc": {"type": "string", "enum": ["2.0"]},
                    "result": {"type": "object"},
                    "error": {"$ref": "#/components/schemas/JSONRPCError"},
                    "id": {
                        "oneOf": [
                            {"type": "string"},
                            {"type": "number"},
                            {"type": "null"}
                        ]
                    }
                }
            },
            "JSONRPCBatchResponse": {
                "type": "array",
                "items": {"$ref": "#/components/schemas/JSONRPCResponse"}
            },
            "JSONRPCError": {
                "type": "object",
                "required": ["code", "message"],
                "properties": {
                    "code": {"type": "integer"},
                    "message": {"type": "string"},
                    "data": {"type": "object"}
                }
            },
            "AgentCard": {
                "type": "object",
                "required": ["agentId", "displayName"],
                "properties": {
                    "agentId": {"type": "string"},
                    "displayName": {"type": "string"},
                    "description": {"type": "string"},
                    "version": {"type": "string"},
                    "capabilities": {"$ref": "#/components/schemas/AgentCapabilities"},
                    "skills": {
                        "type": "array",
                        "items": {"$ref": "#/components/schemas/AgentSkill"}
                    },
                    "extensions": {
                        "type": "array",
                        "items": {"$ref": "#/components/schemas/AgentExtension"}
                    },
                    "provider": {"$ref": "#/components/schemas/AgentProvider"}
                }
            },
            "AgentCapabilities": {
                "type": "object",
                "properties": {
                    "streaming": {"type": "boolean"},
                    "pushNotifications": {"type": "boolean"}
                }
            },
            "AgentSkill": {
                "type": "object",
                "required": ["id", "displayName"],
                "properties": {
                    "id": {"type": "string"},
                    "displayName": {"type": "string"},
                    "description": {"type": "string"},
                    "inputModes": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "outputModes": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                }
            },
            "AgentExtension": {
                "type": "object",
                "required": ["id", "displayName"],
                "properties": {
                    "id": {"type": "string"},
                    "displayName": {"type": "string"},
                    "description": {"type": "string"},
                    "endpoint": {"type": "string"}
                }
            },
            "AgentProvider": {
                "type": "object",
                "properties": {
                    "organization": {"type": "string"},
                    "url": {"type": "string"}
                }
            },
            "HealthCheckResponse": {
                "type": "object",
                "properties": {
                    "status": {"type": "string", "enum": ["healthy", "degraded", "unhealthy"]},
                    "timestamp": {"type": "string", "format": "date-time"},
                    "version": {"type": "string"},
                    "uptimeSeconds": {"type": "number"},
                    "components": {
                        "type": "array",
                        "items": {"$ref": "#/components/schemas/ComponentHealth"}
                    }
                }
            },
            "ComponentHealth": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "status": {"type": "string", "enum": ["healthy", "degraded", "unhealthy"]},
                    "message": {"type": "string"}
                }
            },
            "ReadinessCheckResponse": {
                "type": "object",
                "properties": {
                    "ready": {"type": "boolean"},
                    "message": {"type": "string"}
                }
            },
            "LivenessCheckResponse": {
                "type": "object",
                "properties": {
                    "alive": {"type": "boolean"},
                    "timestamp": {"type": "string", "format": "date-time"}
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_builder() {
        let builder = OpenApiBuilder::new()
            .with_title("Test Service".to_string())
            .with_version("1.0.0".to_string())
            .add_server("http://localhost:8080".to_string(), Some("Local".to_string()))
            .include_health(true);

        let spec = builder.build();

        assert_eq!(spec["openapi"], "3.0.0");
        assert_eq!(spec["info"]["title"], "Test Service");
        assert_eq!(spec["info"]["version"], "1.0.0");
        assert!(spec["paths"]["/health"].is_object());
        assert!(spec["paths"]["/ready"].is_object());
        assert!(spec["paths"]["/live"].is_object());
        assert!(spec["paths"]["/openapi.json"].is_object());
    }

    #[test]
    fn test_openapi_security_schemes() {
        let builder = OpenApiBuilder::new()
            .add_http_security("bearer".to_string(), Some("Bearer auth".to_string()))
            .add_api_key_security(
                "x-api-key".to_string(),
                "header".to_string(),
                Some("API key".to_string()),
            );

        let spec = builder.build();

        assert!(spec["components"]["securitySchemes"]["scheme_0"].is_object());
        assert!(spec["components"]["securitySchemes"]["scheme_1"].is_object());
    }

    #[test]
    fn test_openapi_schemas() {
        let builder = OpenApiBuilder::new();
        let spec = builder.build();
        let schemas = &spec["components"]["schemas"];

        assert!(schemas["JSONRPCRequest"].is_object());
        assert!(schemas["AgentCard"].is_object());
        assert!(schemas["AgentSkill"].is_object());
        assert!(schemas["HealthCheckResponse"].is_object());
    }
}
