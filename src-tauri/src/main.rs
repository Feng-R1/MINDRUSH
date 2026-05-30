// MindRush - Rust Translation App
// Main entry point

use mindrush::application::AppState;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::DEBUG.into())
                .add_directive("pdf_oxide=warn".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting MindRush application");

    // Run Tauri application
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            tracing::info!("MindRush setup complete");
            Ok(())
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            mindrush::application::translate_text,
            mindrush::application::get_cache_stats,
            mindrush::application::get_llm_config,
            mindrush::application::update_llm_config,
            mindrush::application::list_llm_configs,
            mindrush::application::add_llm_config,
            mindrush::application::remove_llm_config,
            mindrush::application::set_active_llm,
            mindrush::application::add_history,
            mindrush::application::get_history,
            mindrush::application::clear_history,
            mindrush::application::export_history,
            mindrush::application::export_batch_results,
            mindrush::application::export_file_results,
            mindrush::application::save_config,
            mindrush::application::load_config,
            mindrush::application::update_system_prompt,
            mindrush::application::save_glossary,
            mindrush::application::load_glossary,
            mindrush::application::read_file,
            mindrush::application::read_epub,
            mindrush::application::export_epub,
            mindrush::application::translate_epub,
            mindrush::application::read_md,
            mindrush::application::export_md,
            mindrush::application::read_pdf,
            mindrush::application::export_pdf,
            mindrush::application::translate_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Simple greeting command to verify Tauri is working
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! MindRush is working.", name)
}