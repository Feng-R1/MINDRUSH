// Application: History and File commands

use crate::application::state::AppState;
use crate::infrastructure::llm::LLMConfigEntry;
use tauri::{State, command};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub original: String,
    pub translated: String,
    pub source_lang: String,
    pub target_lang: String,
    pub tokens: u32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResultItem {
    pub original: String,
    pub translated: String,
    pub source_lang: String,
    pub target_lang: String,
    pub tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub system_prompt: String,
    #[serde(default)]
    pub glossary: String,
    #[serde(default)]
    pub llm_configs: Vec<LLMConfigEntry>,
    #[serde(default)]
    pub active_llm_index: usize,
}

/// Add a translation to history
#[command]
pub async fn add_history(
    state: State<'_, AppState>,
    original: String,
    translated: String,
    source_lang: String,
    target_lang: String,
    tokens: u32,
) -> Result<(), String> {
    let item = HistoryItem {
        original,
        translated,
        source_lang,
        target_lang,
        tokens,
        timestamp: chrono_lite_timestamp(),
    };

    let mut history = state.history.write().await;
    history.push(item);

    // Keep only last 1000 items
    if history.len() > 1000 {
        history.drain(0..100);
    }

    Ok(())
}

/// Get all history items
#[command]
pub async fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryItem>, String> {
    let history = state.history.read().await;
    Ok(history.clone())
}

/// Clear all history
#[command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let mut history = state.history.write().await;
    history.clear();
    Ok(())
}

/// Export history to JSON file
#[command]
pub async fn export_history(state: State<'_, AppState>) -> Result<String, String> {
    use std::fs;

    let history = state.history.read().await;

    let export_dir = get_export_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("mindrush_history_{}.json", timestamp);
    let filepath = export_dir.join(filename);

    let json = serde_json::to_string_pretty(&*history).map_err(|e| e.to_string())?;

    fs::write(&filepath, json).map_err(|e| e.to_string())?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Read file content by path
#[command]
pub async fn read_file(path: String) -> Result<String, String> {
    use std::fs;
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Export batch translation results
#[command]
pub async fn export_batch_results(results: Vec<BatchResultItem>) -> Result<String, String> {
    use std::fs;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("mindrush_batch_{}.txt", timestamp);

    let export_dir = get_export_dir();
    let filepath = export_dir.join(filename);

    let content: String = results
        .iter()
        .map(|r| {
            format!(
                "[Original]\n{}\n[Translated]\n{}\n---",
                r.original, r.translated
            )
        })
        .collect();

    fs::write(&filepath, content).map_err(|e| e.to_string())?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Export file translation results
#[command]
pub async fn export_file_results(content: String, original_filename: String) -> Result<String, String> {
    use std::fs;

    // Create output filename: translated_<original_name>
    // If original_filename is empty (e.g., page was reloaded), use timestamp fallback
    let output_name = if original_filename.is_empty() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("mindrush_translated_{}.txt", timestamp)
    } else {
        format!("translated_{}", original_filename)
    };

    let export_dir = get_export_dir();
    let filepath = export_dir.join(output_name);

    fs::write(&filepath, content).map_err(|e| e.to_string())?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Save config to JSON file
#[command]
pub async fn save_config(state: State<'_, AppState>) -> Result<(), String> {
    use std::fs;

    let configs = state.llm_configs.read().await;
    let active_index = *state.active_llm_index.read().await;
    let system_prompt = state.system_prompt.read().await.clone();
    let glossary = state.glossary.read().await.clone();

    let config_file = ConfigFile {
        system_prompt,
        glossary,
        llm_configs: configs.clone(),
        active_llm_index: active_index,
    };

    let filepath = get_config_dir().join("mindrush_config.json");

    let json = serde_json::to_string_pretty(&config_file).map_err(|e| e.to_string())?;

    fs::write(&filepath, json).map_err(|e| e.to_string())?;

    Ok(())
}

/// Load config from JSON file
#[command]
pub async fn load_config(state: State<'_, AppState>) -> Result<Option<ConfigFile>, String> {
    use std::fs;

    let filepath = get_config_dir().join("mindrush_config.json");

    if !filepath.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&filepath).map_err(|e| e.to_string())?;

    let mut config_file: ConfigFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // Restore state: llm_configs and active index
    {
        let mut configs = state.llm_configs.write().await;
        *configs = config_file.llm_configs.clone();
    }
    {
        let mut idx = state.active_llm_index.write().await;
        *idx = config_file.active_llm_index;
    }
    // Restore state: system_prompt
    {
        let mut sp = state.system_prompt.write().await;
        *sp = config_file.system_prompt.clone();
    }
    // Restore state: glossary (fall back to old separate file if empty in config)
    {
        if config_file.glossary.is_empty() {
            let glossary_path = get_config_dir().join("mindrush_glossary.json");
            if glossary_path.exists() {
                if let Ok(glossary_content) = fs::read_to_string(&glossary_path) {
                    config_file.glossary = glossary_content;
                }
            }
        }
        let mut gl = state.glossary.write().await;
        *gl = config_file.glossary.clone();
    }

    Ok(Some(config_file))
}

/// Update system prompt
#[command]
pub async fn update_system_prompt(
    state: State<'_, AppState>,
    prompt: String,
) -> Result<(), String> {
    let mut sp = state.system_prompt.write().await;
    *sp = prompt;
    Ok(())
}

/// Save glossary to JSON file
#[command]
pub async fn save_glossary(state: State<'_, AppState>, glossary: String) -> Result<(), String> {
    use std::fs;

    *state.glossary.write().await = glossary.clone();

    let filepath = get_config_dir().join("mindrush_glossary.json");
    fs::write(&filepath, glossary).map_err(|e| e.to_string())?;

    Ok(())
}

/// Load glossary from JSON file
#[command]
pub async fn load_glossary(state: State<'_, AppState>) -> Result<String, String> {
    use std::fs;

    let filepath = get_config_dir().join("mindrush_glossary.json");

    if !filepath.exists() {
        return Ok(String::new());
    }

    let content = fs::read_to_string(&filepath).map_err(|e| e.to_string())?;

    *state.glossary.write().await = content.clone();

    Ok(content)
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn get_export_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let export_dir = base.join("MindRush").join("export");
        if !export_dir.exists() {
            let _ = std::fs::create_dir_all(&export_dir);
        }
        export_dir
    }
    #[cfg(not(target_os = "windows"))]
    {
        let base = std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".config"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let export_dir = base.join("MindRush").join("export");
        if !export_dir.exists() {
            let _ = std::fs::create_dir_all(&export_dir);
        }
        export_dir
    }
}

fn get_config_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = base.join("MindRush").join("config");
        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
        }
        config_dir
    }
    #[cfg(not(target_os = "windows"))]
    {
        let base = std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".config"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = base.join("MindRush").join("config");
        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
        }
        config_dir
    }
}
