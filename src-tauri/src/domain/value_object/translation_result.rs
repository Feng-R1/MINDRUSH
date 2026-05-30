// Domain value object: Translation Result

use serde::{Deserialize, Serialize};

/// Translation result from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub translated_text: String,
    pub think_content: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model: String,
    pub finish_reason: Option<String>,
}

impl TranslationResult {
    pub fn new(
        translated_text: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        model: String,
    ) -> Self {
        Self {
            translated_text,
            think_content: None,
            prompt_tokens,
            completion_tokens,
            model,
            finish_reason: None,
        }
    }

    pub fn total_tokens(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}