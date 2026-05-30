// Application: Config commands

use crate::application::state::AppState;
use crate::infrastructure::llm::{LLMConfig, LLMConfigEntry, ProviderType};
use tauri::{command, State};

/// Get the active LLM configuration
#[command]
pub async fn get_llm_config(state: State<'_, AppState>) -> Result<LLMConfig, String> {
    let configs = state.llm_configs.read().await;
    let index = *state.active_llm_index.read().await;
    configs
        .get(index)
        .map(|entry| entry.config.clone())
        .ok_or_else(|| "No active LLM configuration found".to_string())
}

/// Update the active LLM configuration
#[command]
pub async fn update_llm_config(
    state: State<'_, AppState>,
    provider: String,
    model: String,
    api_key: String,
    base_url: Option<String>,
    temperature: f32,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    let provider_type = match provider.to_lowercase().as_str() {
        "openai" => ProviderType::OpenAI,
        "anthropic" => ProviderType::Anthropic,
        "google" => ProviderType::Google,
        "bedrock" => ProviderType::AmazonBedrock,
        "local" => ProviderType::LocalLLM,
        "sakura" => ProviderType::Sakura,
        _ => return Err("Unknown provider".to_string()),
    };

    let config = LLMConfig {
        provider_type,
        model,
        api_key,
        base_url,
        max_tokens,
        temperature,
        timeout_secs: 120,
    };

    let mut configs = state.llm_configs.write().await;
    let index = *state.active_llm_index.read().await;
    if let Some(entry) = configs.get_mut(index) {
        entry.config = config;
        Ok(())
    } else {
        Err("No active LLM configuration found".to_string())
    }
}

/// List all LLM configurations
#[command]
pub async fn list_llm_configs(state: State<'_, AppState>) -> Result<Vec<LLMConfigEntry>, String> {
    Ok(state.llm_configs.read().await.clone())
}

/// Add a new LLM configuration and set it as active
#[command]
pub async fn add_llm_config(
    state: State<'_, AppState>,
    name: String,
    provider: String,
    model: String,
    api_key: String,
    base_url: Option<String>,
    temperature: f32,
    max_tokens: Option<u32>,
) -> Result<LLMConfigEntry, String> {
    let provider_type = match provider.to_lowercase().as_str() {
        "openai" => ProviderType::OpenAI,
        "anthropic" => ProviderType::Anthropic,
        "google" => ProviderType::Google,
        "bedrock" => ProviderType::AmazonBedrock,
        "local" => ProviderType::LocalLLM,
        "sakura" => ProviderType::Sakura,
        _ => return Err("Unknown provider".to_string()),
    };

    let config = LLMConfig {
        provider_type,
        model,
        api_key,
        base_url,
        max_tokens,
        temperature,
        timeout_secs: 120,
    };

    let entry = LLMConfigEntry { name, config };

    let mut configs = state.llm_configs.write().await;
    configs.push(entry.clone());
    *state.active_llm_index.write().await = configs.len() - 1;

    Ok(entry)
}

/// Remove an LLM configuration by index
#[command]
pub async fn remove_llm_config(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    let mut configs = state.llm_configs.write().await;

    if index >= configs.len() {
        return Err(format!(
            "Index out of bounds: {} (max {})",
            index,
            configs.len().saturating_sub(1)
        ));
    }

    if configs.len() <= 1 {
        return Err("Cannot remove the last LLM configuration".to_string());
    }

    configs.remove(index);

    // Adjust active_llm_index if needed
    let mut active_index = state.active_llm_index.write().await;
    if *active_index >= configs.len() && configs.len() > 0 {
        *active_index = configs.len() - 1;
    } else if index < *active_index {
        *active_index -= 1;
    }

    Ok(())
}

/// Set the active LLM configuration by index
#[command]
pub async fn set_active_llm(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    let configs = state.llm_configs.read().await;
    if index >= configs.len() {
        return Err(format!(
            "Index out of bounds: {} (max {})",
            index,
            configs.len().saturating_sub(1)
        ));
    }
    drop(configs); // Release read lock before acquiring write lock

    *state.active_llm_index.write().await = index;
    Ok(())
}
