// Infrastructure: Anthropic LLM Provider implementation

use super::{LLMConfig, LLMProvider, LLMError, Message, MessageRole};
use crate::domain::value_object::TranslationResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub struct AnthropicProvider {
    client: Client,
    config: LLMConfig,
}

impl AnthropicProvider {
    pub fn new(config: LLMConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }
}

impl LLMProvider for AnthropicProvider {
    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn complete<'a>(
        &'a self,
        messages: Vec<Message>,
        system_prompt: &'a str,
        config: &'a LLMConfig,
    ) -> Pin<Box<dyn Future<Output = Result<TranslationResult, LLMError>> + Send + 'a>> {
        let client = self.client.clone();
        let full_url = config.base_url.as_deref()
            .map(|url| format!("{}/v1/messages", url.trim_end_matches('/')))
            .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());

        Box::pin(async move {
            let request_body = AnthropicRequest {
                model: &config.model,
                system: (!system_prompt.is_empty()).then(|| system_prompt.to_string()),
                messages: messages.iter().map(|m| AnthropicMessage {
                    role: match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        _ => "user",
                    }.to_string(),
                    content: m.content.clone(),
                }).collect(),
                max_tokens: config.max_tokens.unwrap_or(4096),
            };

            let response = client
                .post(&full_url)
                .header("x-api-key", &config.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| LLMError::RequestFailed(e.to_string()))?;

            if response.status().as_u16() == 401 {
                return Err(LLMError::AuthFailed);
            }
            if response.status().as_u16() == 429 {
                return Err(LLMError::RateLimited);
            }

            let response_body: AnthropicResponse = response
                .json()
                .await
                .map_err(|e| LLMError::ParseResponseFailed(e.to_string()))?;

            let content = response_body.content
                .iter()
                .find(|c| c.type_ == "text")
                .and_then(|c| c.text.clone())
                .unwrap_or_default();

            Ok(TranslationResult {
                translated_text: content,
                think_content: None,
                prompt_tokens: response_body.usage.input_tokens as u32,
                completion_tokens: response_body.usage.output_tokens as u32,
                model: config.model.clone(),
                finish_reason: None,
            })
        })
    }
}

#[derive(Debug, Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
}