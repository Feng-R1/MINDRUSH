// Application: Translate commands

use crate::application::state::AppState;
use crate::domain::entity::{CacheItem, Language};
use crate::domain::value_object::TranslationResult;
use crate::infrastructure::llm::{LLMConfig, LLMProviderFactory, with_retry, Message, MessageRole};
use tauri::{command, State};

/// Cache statistics response
#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub size: usize,
    pub hit_rate: f64,
}

fn translation_cache_key(
    text: &str,
    source_lang: &str,
    target_lang: &str,
    config: &LLMConfig,
    system_prompt: &str,
    glossary: &str,
    previous_text: Option<&str>,
) -> String {
    CacheItem::compute_hash(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        text,
        source_lang,
        target_lang,
        config.provider_type as u8,
        config.model,
        config.base_url.as_deref().unwrap_or_default(),
        config.temperature,
        config.max_tokens.unwrap_or_default(),
        system_prompt,
        glossary,
        previous_text.unwrap_or_default()
    ))
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
    if text.trim().is_empty() {
        return Err("请输入需要翻译的文本".into());
    }
    // Cache by every setting that affects translation, so language, model and
    // glossary changes cannot return a stale translation for the same text.
    let active_index = *state.active_llm_index.read().await;
    let llm_config = state
        .llm_configs
        .read()
        .await
        .get(active_index)
        .map(|entry| entry.config.clone())
        .ok_or_else(|| "No active LLM configuration found".to_string())?;
    let system_prompt = state.system_prompt.read().await.clone();
    let glossary = state.glossary.read().await.clone();
    llm_config.validate().map_err(|error| error.to_string())?;
    let source = Language::from_code(&source_lang).ok_or("Invalid source language")?;
    let target = Language::from_code(&target_lang).ok_or("Invalid target language")?;
    let cache_key = translation_cache_key(
        &text,
        source.to_ietf_tag(),
        target.to_ietf_tag(),
        &llm_config,
        &system_prompt,
        &glossary,
        previous_text.as_deref(),
    );
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

#[cfg(test)]
mod tests {
    use super::translation_cache_key;
    use crate::infrastructure::llm::{LLMConfig, ProviderType};

    fn config() -> LLMConfig {
        LLMConfig {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4o-mini".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            max_tokens: Some(4096),
            temperature: 0.3,
            timeout_secs: 120,
        }
    }

    fn key(config: &LLMConfig, target: &str, glossary: &str) -> String {
        translation_cache_key("hello", "en", target, config, "translator", glossary, None)
    }

    #[test]
    fn cache_key_changes_when_translation_settings_change() {
        let original = config();
        let original_key = key(&original, "zh-Hans", "");

        let mut changed_model = original.clone();
        changed_model.model = "gpt-4.1".to_string();
        let mut changed_provider = original.clone();
        changed_provider.provider_type = ProviderType::Anthropic;
        let mut changed_url = original.clone();
        changed_url.base_url = Some("http://localhost:8080/v1".to_string());

        assert_ne!(original_key, key(&original, "zh-Hant", ""));
        assert_ne!(original_key, key(&changed_model, "zh-Hans", ""));
        assert_ne!(original_key, key(&changed_provider, "zh-Hans", ""));
        assert_ne!(original_key, key(&changed_url, "zh-Hans", ""));
        assert_ne!(original_key, key(&original, "zh-Hans", "MindRush=MindRush"));
        assert_eq!(original_key, key(&original, "zh-Hans", ""));
    }
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
