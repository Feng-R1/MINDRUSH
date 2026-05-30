// Infrastructure: LLM Provider trait and types

use std::future::Future;
use std::pin::Pin;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Provider type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProviderType {
    OpenAI = 0,
    Anthropic = 1,
    Google = 2,
    AmazonBedrock = 3,
    LocalLLM = 4,
    Sakura = 5,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider_type: ProviderType,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: f32,
    pub timeout_secs: u64,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4".to_string(),
            api_key: String::new(),
            base_url: None,
            max_tokens: Some(4096),
            temperature: 0.7,
            timeout_secs: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfigEntry {
    pub name: String,
    pub config: LLMConfig,
}

/// Chat message
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// LLM error types
#[derive(Debug, Error, Clone)]
pub enum LLMError {
    #[error("API request failed: {0}")]
    RequestFailed(String),

    #[error("Failed to parse API response: {0}")]
    ParseResponseFailed(String),

    #[error("Rate limited: please retry later")]
    RateLimited,

    #[error("Authentication failed: check API key")]
    AuthFailed,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

/// LLM Provider trait - all LLM providers must implement
pub trait LLMProvider: Send + Sync {
    fn complete<'a>(
        &'a self,
        messages: Vec<Message>,
        system_prompt: &'a str,
        config: &'a LLMConfig,
    ) -> Pin<Box<dyn Future<Output = Result<crate::domain::value_object::TranslationResult, LLMError>> + Send + 'a>>;

    fn provider_name(&self) -> &'static str;

    fn validate_config(&self, config: &LLMConfig) -> Result<(), LLMError> {
        if config.api_key.is_empty() {
            return Err(LLMError::InvalidConfig("API key is required".to_string()));
        }
        if config.model.is_empty() {
            return Err(LLMError::InvalidConfig("Model is required".to_string()));
        }
        Ok(())
    }
}