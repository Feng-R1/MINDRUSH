// Infrastructure: OpenAI LLM Provider implementation

use super::{LLMConfig, LLMProvider, LLMError, Message, MessageRole};
use crate::domain::value_object::TranslationResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub struct OpenAIProvider {
    client: Client,
    config: LLMConfig,
}

impl OpenAIProvider {
    pub fn new(config: LLMConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }
}

impl LLMProvider for OpenAIProvider {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn complete<'a>(
        &'a self,
        messages: Vec<Message>,
        system_prompt: &'a str,
        config: &'a LLMConfig,
    ) -> Pin<Box<dyn Future<Output = Result<TranslationResult, LLMError>> + Send + 'a>> {
        let client = self.client.clone();
        let full_url = config.base_url.as_deref()
            .map(|url| format!("{}/chat/completions", url.trim_end_matches('/')))
            .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string());

        Box::pin(async move {
            // Build request body
            let mut body_messages = vec![Message {
                role: MessageRole::System,
                content: system_prompt.to_string(),
            }];
            body_messages.extend(messages);

            let request_body = OpenAIRequest {
                model: &config.model,
                messages: body_messages.iter().map(|m| OpenAIMessage {
                    role: match m.role {
                        MessageRole::System => "system",
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    },
                    content: &m.content,
                }).collect(),
                max_tokens: config.max_tokens.unwrap_or(4096),
                temperature: config.temperature,
            };

            // Send request
            let response = client
                .post(&full_url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| LLMError::RequestFailed(e.to_string()))?;

            // Check HTTP status
            let status = response.status();
            if status.as_u16() == 401 {
                return Err(LLMError::AuthFailed);
            }
            if status.as_u16() == 429 {
                return Err(LLMError::RateLimited);
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(LLMError::RequestFailed(format!("HTTP {}: {}", status, body)));
            }

            // Parse response
            let response_body: OpenAIResponse = response
                .json()
                .await
                .map_err(|e| LLMError::ParseResponseFailed(e.to_string()))?;

            // Extract result
            let choice = response_body.choices.first()
                .ok_or_else(|| LLMError::InvalidResponse("No choices in response".to_string()))?;

            let content = choice.message.content.as_deref()
                .unwrap_or("")
                .to_string();

            Ok(TranslationResult {
                translated_text: content,
                think_content: None,
                prompt_tokens: response_body.usage.prompt_tokens as u32,
                completion_tokens: response_body.usage.completion_tokens as u32,
                model: config.model.clone(),
                finish_reason: choice.finish_reason.clone(),
            })
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAIMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessageContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessageContent {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}