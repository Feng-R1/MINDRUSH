// Application: Translate commands

use crate::application::state::AppState;
use crate::domain::entity::{CacheItem, Language};
use crate::domain::value_object::TranslationResult;
use crate::infrastructure::llm::{LLMProviderFactory, with_retry, Message, MessageRole};
use tauri::{command, State};

/// Cache statistics response
#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub size: usize,
    pub hit_rate: f64,
}

/// Translate a single text
#[command]
pub async fn translate_text(
    state: State<'_, AppState>,
    text: String,
    source_lang: String,
    target_lang: String,
    previous_text: Option<String>,
) -> Result<TranslationResult, String> {
    // Check cache first
    let cache_key = CacheItem::compute_hash(&text);
    {
        let cache = state.cache.read().await;
        if let Some(item) = cache.get(&cache_key) {
            if item.is_translated() {
                return Ok(TranslationResult {
                    translated_text: item.translated_text.clone(),
                    think_content: None,
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    model: item.model.clone(),
                    finish_reason: None,
                });
            }
        }
    }

    // Get language
    let source = Language::from_code(&source_lang).ok_or("Invalid source language")?;
    let target = Language::from_code(&target_lang).ok_or("Invalid target language")?;

    // Build messages
    let user_message = Message {
        role: MessageRole::User,
        content: format!("Translate the following from {} to {}:\n\n{}", source.to_ietf_tag(), target.to_ietf_tag(), text),
    };

    let mut messages = Vec::new();
    if let Some(prev) = previous_text {
        messages.push(Message {
            role: MessageRole::Assistant,
            content: format!("[Previous translation]:\n{}", prev),
        });
    }
    messages.push(user_message);

    // Get LLM provider and system prompt
    let active_index = *state.active_llm_index.read().await;
    let llm_config = state.llm_configs.read().await[active_index].config.clone();
    let system_prompt = state.system_prompt.read().await.clone();
    let glossary = state.glossary.read().await.clone();
    let effective_system_prompt = if glossary.trim().is_empty() {
        system_prompt.clone()
    } else {
        format!("{}\n\n[Terminology Glossary - follow these term mappings precisely]:\n{}", system_prompt, glossary)
    };
    let provider = LLMProviderFactory::create(&llm_config);

    // Send request with retry
    let result = with_retry(
        || provider.complete(messages.clone(), &effective_system_prompt, &llm_config),
        3,
        1000,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Update cache
    {
        let mut cache = state.cache.write().await;
        let mut item = CacheItem::new(text, 0);
        item.translated_text = result.translated_text.clone();
        item.model = result.model.clone();
        item.tokens_used = result.total_tokens();
        item.translation_status = crate::domain::entity::TranslationStatus::Translated;
        cache.set(cache_key, item);
    }

    Ok(result)
}

/// Get cache statistics
#[command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> Result<CacheStats, String> {
    let cache = state.cache.read().await;
    Ok(CacheStats {
        size: cache.len(),
        hit_rate: 0.0, // Would need more state tracking to compute properly
    })
}