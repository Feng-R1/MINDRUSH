// Infrastructure: LLM Provider factory

use super::{LLMConfig, LLMProvider, ProviderType};
use super::openai::OpenAIProvider;
use std::sync::Arc;
use std::future::Future;

/// LLM Provider factory
pub struct LLMProviderFactory;

impl LLMProviderFactory {
    /// Create provider based on config
    pub fn create(config: &LLMConfig) -> Arc<dyn LLMProvider> {
        match config.provider_type {
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.clone())),
            ProviderType::Anthropic => Arc::new(super::anthropic::AnthropicProvider::new(config.clone())),
            _ => Arc::new(OpenAIProvider::new(config.clone())),
        }
    }
}

/// Execute with retry logic
pub async fn with_retry<F, Fut, T>(
    mut f: F,
    max_retries: u32,
    initial_delay_ms: u64,
) -> Result<T, super::LLMError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, super::LLMError>>,
{
    let mut delay = initial_delay_ms;
    let mut last_error = None;

    for attempt in 0..max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e.clone());
                match e {
                    super::LLMError::RateLimited | super::LLMError::Timeout(_) => {
                        if attempt < max_retries - 1 {
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                            delay *= 2; // Exponential backoff
                        }
                    }
                    _ => return Err(e),
                }
            }
        }
    }

    Err(last_error.unwrap_or(super::LLMError::RequestFailed("Max retries exceeded".to_string())))
}