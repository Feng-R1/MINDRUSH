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
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
            base_url: None,
            max_tokens: Some(4096),
            temperature: 0.3,
            timeout_secs: 120,
        }
    }
}

impl LLMConfig {
    pub fn validate(&self) -> Result<(), LLMError> {
        if self.model.trim().is_empty() {
            return Err(LLMError::InvalidConfig("请填写模型名称".into()));
        }
        if matches!(self.provider_type, ProviderType::LocalLLM | ProviderType::Sakura) {
            if self.base_url.as_deref().is_none_or(|url| url.trim().is_empty()) {
                return Err(LLMError::InvalidConfig("本地模型需要填写接口地址".into()));
            }
        } else if self.api_key.trim().is_empty() {
            return Err(LLMError::InvalidConfig("请先在模型设置中填写 API Key".into()));
        }
        if let Some(url) = self.base_url.as_deref() {
            let url = reqwest::Url::parse(url)
                .map_err(|_| LLMError::InvalidConfig("接口地址无效".into()))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
                || !url.username().is_empty() || url.password().is_some()
                || url.query().is_some() || url.fragment().is_some()
            {
                return Err(LLMError::InvalidConfig("接口地址需为不含凭据、查询参数的 HTTP(S) 基础地址".into()));
            }
        }
        if !self.temperature.is_finite() || !(0.0..=2.0).contains(&self.temperature)
            || self.max_tokens == Some(0) || self.timeout_secs == 0
        {
            return Err(LLMError::InvalidConfig("采样参数或超时无效".into()));
        }
        Ok(())
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

    #[error("服务请求过于频繁，请稍后重试")]
    RateLimited,

    #[error("模型服务认证失败，请检查 API Key 和服务权限")]
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
        config.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::{LLMConfig, ProviderType};

    #[test]
    fn local_models_require_an_explicit_endpoint_but_no_key() {
        let mut config = LLMConfig::default();
        config.provider_type = ProviderType::LocalLLM;
        assert!(config.validate().is_err());
        config.base_url = Some("http://localhost:11434/v1".into());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn cloud_models_require_a_key_and_a_valid_base_url() {
        let mut config = LLMConfig::default();
        assert!(config.validate().is_err());
        config.api_key = "fixture-only".into();
        assert!(config.validate().is_ok());
        for bad in ["file:///tmp/model", "https://token@example.com", "https://example.com?key=secret"] {
            config.base_url = Some(bad.into());
            assert!(config.validate().is_err());
        }
    }
}
