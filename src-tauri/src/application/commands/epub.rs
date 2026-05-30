// Application: EPUB commands

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

use regex::Regex;
use rbook::read::ContentType;
use rbook::Ebook;
use tauri::{command, Emitter, State};

use crate::application::state::{AppState, EpubDocument, EpubLoadInfo, MarkedSection, SegmentMeta, SegmentType};
use crate::domain::entity::Language;
use crate::infrastructure::llm::{LLMProviderFactory, with_retry, Message, MessageRole};
use lol_html::{element, end_tag, rewrite_str, text, RewriteStrSettings};

/// Read and parse an EPUB file.
///
/// Opens the EPUB, iterates through spine sections via `reader()`,
/// extracts text from translatable elements and inserts placeholder
/// markers via a single lol_html streaming rewrite pass, ensuring
/// extraction and marking are always aligned.
/// Stores the parsed document in `AppState.epub_document`.
#[command]
pub async fn read_epub(
    state: State<'_, AppState>,
    path: String,
) -> Result<EpubLoadInfo, String> {
    // Process EPUB synchronously in a block to ensure non-Send rbook types
    // (which contain Rc internally) are dropped before the .await point.
    let (doc, load_info) = {
        let epub = rbook::Epub::new(&path)
            .map_err(|e| format!("Failed to open EPUB: {}", e))?;

        let filename = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown.epub".to_string());

        // CSS selector for translatable elements
        let selector =
            "p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote, figcaption, dt, dd, caption";

        let reader = epub.reader();
        let mut all_segments: Vec<String> = Vec::new();
        let mut marked_sections: Vec<MarkedSection> = Vec::new();
        let mut global_idx: usize = 0;

        for data_result in reader.iter() {
            let data = data_result.map_err(|e| format!("Failed to read section: {}", e))?;
            let section_path = data
                .get_content(ContentType::Path)
                .unwrap_or("")
                .to_string();
            let original_html = data.as_lossy_str().to_string();

            let segment_start = all_segments.len();

            /// Tracks per-section state for the lol_html rewrite pass.
            struct SectionState {
                code_block_depth: usize,
                pending_text: String,
                has_pending_element: bool,
                segments: Vec<String>,
                marker_idx: usize,
            }

            // Use lol_html for BOTH text extraction AND marker insertion in one pass.
            // This ensures extraction and marking are always aligned — they come
            // from the same DOM traversal, eliminating the misalignment bug from
            // the regex-based approach.
            let state = Rc::new(RefCell::new(SectionState {
                code_block_depth: 0,
                pending_text: String::new(),
                has_pending_element: false,
                segments: Vec::new(),
                marker_idx: global_idx,
            }));

            let marked_xhtml = {
                let s_track = state.clone();
                let s_main = state.clone();
                let s_text = state.clone();

                rewrite_str(
                    &original_html,
                    RewriteStrSettings {
                        element_content_handlers: vec![
                            // Track nesting depth of <pre> and <code> blocks
                            // so we can skip translatable elements inside them.
                            element!("pre, code", move |el| {
                                s_track.borrow_mut().code_block_depth += 1;
                                let s = s_track.clone();
                                el.on_end_tag(end_tag!(move |_end| {
                                    s.borrow_mut().code_block_depth -= 1;
                                    Ok(())
                                }))
                                .map_err(|_| "on_end_tag failed for pre/code")?;
                                Ok(())
                            }),
                            // Main handler: clear content and register end-tag handler
                            // for translatable elements (p, h1-h6, li, etc.)
                            element!(selector, move |el| {
                                let mut st = s_main.borrow_mut();

                                // Skip elements inside code blocks
                                if st.code_block_depth > 0 {
                                    return Ok(());
                                }

                                // Clear the element's inner content (original text).
                                // The marker will be inserted in the end-tag handler
                                // if the accumulated text is non-empty.
                                el.set_inner_content(
                                    "",
                                    lol_html::html_content::ContentType::Text,
                                );

                                // Mark that we have a pending element for the text handler
                                st.has_pending_element = true;

                                // Register end-tag handler: finalise accumulated text,
                                // insert marker if non-empty.
                                if el.can_have_content() {
                                    let s = s_main.clone();
                                    el.on_end_tag(end_tag!(move |end| {
                                        let mut st = s.borrow_mut();
                                        let text =
                                            std::mem::take(&mut st.pending_text);
                                        let trimmed = text.trim().to_string();

                                        if !trimmed.is_empty() {
                                            let marker =
                                                format!("\x00EPUB_{}\x00", st.marker_idx);
                                            st.marker_idx += 1;
                                            st.segments.push(trimmed);
                                            end.before(
                                                &marker,
                                                lol_html::html_content::ContentType::Text,
                                            );
                                        }

                                        st.has_pending_element = false;
                                        Ok(())
                                    }))?;
                                }

                                Ok(())
                            }),
                            // Text handler: accumulate text for the current element
                            text!(selector, move |t| {
                                let mut st = s_text.borrow_mut();
                                if st.has_pending_element {
                                    st.pending_text.push_str(t.as_str());
                                }
                                Ok(())
                            }),
                        ],
                        ..RewriteStrSettings::new()
                    },
                )
                .map_err(|e| format!("HTML rewriting error: {}", e))?
            };

            let mut st = state.borrow_mut();
            let segment_count = st.segments.len();
            all_segments.extend(st.segments.drain(..));
            global_idx = st.marker_idx;

            marked_sections.push(MarkedSection {
                spine_path: section_path,
                marked_xhtml,
                segment_start,
                segment_count,
            });
        }

        let total_segments = all_segments.len();
        let doc = EpubDocument {
            original_path: path,
            filename: filename.clone(),
            segments: all_segments,
            translations: vec![None; total_segments],
            marked_sections,
            segment_meta: vec![SegmentMeta {
                segment_type: SegmentType::Paragraph,
                keep_original: false,
                font_size: None,
                is_monospace: false,
                indent: None,
                is_italic: false,
            }; total_segments],
            code_spans: vec![],
        };
        let load_info = EpubLoadInfo {
            filename,
            segment_count: total_segments,
        };

        (doc, load_info)
    }; // rbook types dropped here

    // Store document in state (only await point)
    *state.epub_document.write().await = Some(doc);

    Ok(load_info)
}

/// Emit a translation progress event to the frontend.
///
/// Sends the current batch and segment position to allow progress tracking
/// in the UI without polling.
fn emit_translation_progress(
    app_handle: &tauri::AppHandle,
    current_batch: usize,
    total_batches: usize,
    current_segment: usize,
    total_segments: usize,
) {
    app_handle
        .emit(
            "epub-translation-progress",
            serde_json::json!({
                "current_batch": current_batch,
                "total_batches": total_batches,
                "current_segment": current_segment,
                "total_segments": total_segments,
            }),
        )
        .ok();
}

/// Translate all segments stored in AppState.epub_document.
///
/// Reads segments from state, calls LLM for each, stores translations back.
/// Already-translated segments are skipped.
#[command]
pub async fn translate_epub(
    state: State<'_, AppState>,
    source_lang: String,
    target_lang: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Step 1: Clone segments and existing translations under read lock
    let (segments, existing_translations, segment_meta) = {
        let doc = state.epub_document.read().await;
        let doc = doc.as_ref().ok_or("No EPUB loaded. Please load an EPUB first.")?;

        if doc.translations.iter().all(|t| t.is_some()) {
            return Ok(());
        }

        (doc.segments.clone(), doc.translations.clone(), doc.segment_meta.clone())
    }; // Read lock dropped

    // Step 2: Set up LLM provider and prompts
    let active_index = *state.active_llm_index.read().await;
    let llm_config = state.llm_configs.read().await[active_index].config.clone();
    let system_prompt = state.system_prompt.read().await.clone();
    let glossary = state.glossary.read().await.clone();
    let effective_system_prompt = if glossary.trim().is_empty() {
        system_prompt.clone()
    } else {
        format!(
            "{}\n\n[Terminology Glossary - follow these term mappings precisely]:\n{}",
            system_prompt, glossary
        )
    };

    let provider = LLMProviderFactory::create(&llm_config);

    let source = Language::from_code(&source_lang).ok_or("Invalid source language")?;
    let target = Language::from_code(&target_lang).ok_or("Invalid target language")?;

    // Step 3: Collect untranslated segment indices
    let mut results = existing_translations;
    let mut untranslated: Vec<(usize, &str)> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        if results[i].is_none() {
            // Skip segments that should not be translated (code blocks, etc.)
            if i < segment_meta.len() && segment_meta[i].keep_original {
                results[i] = Some(seg.clone()); // Keep original as "translation"
                continue;
            }
            untranslated.push((i, seg.as_str()));
        }
    }

    if untranslated.is_empty() {
        return Ok(());
    }

    const MAX_BATCH: usize = 100;
    let total_segments = segments.len();
    let mut failed_batches: Vec<usize> = Vec::new();
    let mut llm_attempts: usize = 0;
    let mut llm_successes: usize = 0;
    let mut processed_segments: usize = segments.len() - untranslated.len();

    // Queue of (start_idx, end_idx) in the untranslated list
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    queue.push_back((0, untranslated.len()));

    while let Some((start, end)) = queue.pop_front() {
        // Brief pause between batches to avoid API rate limiting
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // If the chunk is larger than MAX_BATCH, split first (don't even try)
        if end - start > MAX_BATCH {
            let remaining_start = start + MAX_BATCH;
            queue.push_front((remaining_start, end));      // remaining (process later)
            queue.push_front((start, remaining_start));     // first batch (process now)
            continue;
        }

        let batch = &untranslated[start..end];

        // Preemptive split: if total text length exceeds threshold, split without calling LLM
        let total_chars: usize = batch.iter().map(|(_, seg)| seg.len()).sum();
        const MAX_CHARS_PER_BATCH: usize = 6000; // ~4000 tokens (safe for most LLMs)

        if total_chars > MAX_CHARS_PER_BATCH && batch.len() > 1 {
            let mid = start + (end - start) / 2;
            queue.push_front((mid, end));
            queue.push_front((start, mid));
            continue;
        }

        // Emit progress
        app_handle.emit("epub-translation-progress", serde_json::json!({
            "current_batch": 1,
            "total_batches": 1,
            "current_segment": processed_segments,
            "total_segments": total_segments,
            "llm_attempts": llm_attempts,
            "llm_successes": llm_successes,
        })).ok();

        // Build and try
        let prompt = build_batch_prompt(batch, &source, &target);

        // First attempt
        llm_attempts += 1;
        let translations_result = with_retry(
            || {
                provider.complete(
                    vec![Message { role: MessageRole::User, content: prompt.clone() }],
                    &effective_system_prompt,
                    &llm_config,
                )
            },
            1,
            1000,
        ).await;

        let translations_result = match translations_result {
            Ok(resp) => {
                parse_batch_response(&resp.translated_text, batch.len())
                    .map_err(|e| format!("parse failed: {}", e))
            }
            Err(e) => Err(format!("LLM failed: {}", e)),
        };

        let translations: Vec<String> = match translations_result {
            Ok(t) => {
                llm_successes += 1;
                t
            }
            Err(_e) => {
                // Retry with clearer prompt
                let retry_prompt = build_retry_prompt(&prompt);
                llm_attempts += 1;
                let retry_result = with_retry(
                    || {
                        provider.complete(
                            vec![Message { role: MessageRole::User, content: retry_prompt.clone() }],
                            &effective_system_prompt,
                            &llm_config,
                        )
                    },
                    1,
                    1000,
                ).await;

                match retry_result {
                    Ok(resp) => {
                        match parse_batch_response(&resp.translated_text, batch.len()) {
                            Ok(t) => {
                                llm_successes += 1;
                                t
                            }
                            Err(_) => {
                                // Both attempts failed — split or give up
                                if batch.len() == 1 {
                                    // Single segment, can't split further
                                    let (global_idx, _) = batch[0];
                                    app_handle.emit("epub-translation-error", serde_json::json!({
                                        "segment": global_idx,
                                        "error": "Failed to translate segment after retry"
                                    })).ok();
                                    failed_batches.push(global_idx);
                                    processed_segments += 1;
                                    continue;
                                }
                                // Split in half
                                let mid = start + (end - start) / 2;
                                queue.push_front((mid, end));    // second half
                                queue.push_front((start, mid));  // first half
                                continue;
                            }
                        }
                    }
                    Err(_) => {
                        if batch.len() == 1 {
                            let (global_idx, _) = batch[0];
                            app_handle.emit("epub-translation-error", serde_json::json!({
                                "segment": global_idx,
                                "error": "LLM failed for single segment after retry"
                            })).ok();
                            failed_batches.push(global_idx);
                            processed_segments += 1;
                            continue;
                        }
                        let mid = start + (end - start) / 2;
                        queue.push_front((mid, end));
                        queue.push_front((start, mid));
                        continue;
                    }
                }
            }
        };

        // Success: store translations
        for (i, (global_idx, _)) in batch.iter().enumerate() {
            results[*global_idx] = Some(translations[i].clone());
        }
        processed_segments += batch.len();
        save_translations_incrementally(&state, &results).await?;
    }

    // Emit final 100% progress with complete API stats
    app_handle.emit("epub-translation-progress", serde_json::json!({
        "current_batch": 1,
        "total_batches": 1,
        "current_segment": total_segments,
        "total_segments": total_segments,
        "llm_attempts": llm_attempts,
        "llm_successes": llm_successes,
    })).ok();

    if failed_batches.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} of {} segments translated. Failed segments: {:?}. Click 'Translate File' again to retry, or reduce BATCH_SIZE.",
            results.iter().filter(|t| t.is_some()).count(),
            segments.len(),
            failed_batches,
        ))
    }
}

/// Save translations incrementally after each batch completes.
///
/// Acquires the write lock on `state.epub_document` and replaces
/// `doc.translations` with the current `results`, so progress is
/// preserved even if the translation loop crashes later.
pub(crate) async fn save_translations_incrementally(
    state: &State<'_, AppState>,
    results: &[Option<String>],
) -> Result<(), String> {
    let mut doc = state.epub_document.write().await;
    if let Some(ref mut doc) = *doc {
        doc.translations = results.to_vec();
    }
    Ok(())
}

/// Write translated EPUB using markers stored in AppState.
///
/// Loads marked_xhtml and translations from state, replaces all \x00EPUB_N\x00
/// markers with translated text, writes the result as a new EPUB.
#[command]
pub async fn export_epub(state: State<'_, AppState>) -> Result<String, String> {
    // Get document from state
    let (original_path, marked_sections, translations) = {
        let doc = state.epub_document.read().await;
        let doc = doc.as_ref().ok_or("No EPUB loaded. Please load and translate an EPUB first.")?;
        
        // Check all translations are complete
        let incomplete = doc.translations.iter().enumerate().find(|(_, t)| t.is_none());
        if let Some((i, _)) = incomplete {
            return Err(format!("Translation incomplete: segment {} is not translated. Click 'Translate File' to retry remaining segments.", i));
        }
        
        (doc.original_path.clone(), doc.marked_sections.clone(), doc.translations.clone())
    };
    
    let path = Path::new(&original_path);
    let stem = path.file_stem()
        .ok_or_else(|| "Invalid file path: missing file stem".to_string())?
        .to_string_lossy();
    let parent = path.parent().unwrap_or(Path::new("."));
    let output_path = parent.join(format!("{}_translated.epub", stem));
    let output_path_str = output_path.to_string_lossy().to_string();
    
    // Open original EPUB as ZIP
    let file = std::fs::File::open(&original_path)
        .map_err(|e| format!("Failed to open EPUB file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read EPUB as ZIP: {}", e))?;
    let out_file = std::fs::File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let file_options = zip::write::SimpleFileOptions::default();
    
    // Regex to find all EPUB_N markers in the marked_xhtml
    let marker_re = Regex::new(r"\x00EPUB_(\d+)\x00")
        .map_err(|e| format!("Regex compilation error: {}", e))?;
    
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let entry_name = entry.name().to_owned();
        
        // Find matching marked section
        let matching_section = marked_sections.iter()
            .find(|s| entry_name == s.spine_path 
                || entry_name.ends_with(&s.spine_path) 
                || s.spine_path.ends_with(&entry_name));
        
        if let Some(section) = matching_section {
            // Replace markers with translations
            let new_content = marker_re.replace_all(&section.marked_xhtml, |caps: &regex::Captures| {
                let idx = caps.get(1)
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                translations.get(idx).cloned().flatten().unwrap_or_default()
            });
            
            writer.start_file(&entry_name, file_options)
                .map_err(|e| format!("Failed to create entry '{}': {}", entry_name, e))?;
            writer.write_all(new_content.as_bytes())
                .map_err(|e| format!("Failed to write entry '{}': {}", entry_name, e))?;
        } else {
            // Binary entry — copy unchanged
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read binary entry '{}': {}", entry_name, e))?;
            writer.start_file(&entry_name, file_options)
                .map_err(|e| format!("Failed to create entry '{}': {}", entry_name, e))?;
            writer.write_all(&buf)
                .map_err(|e| format!("Failed to write binary entry '{}': {}", entry_name, e))?;
        }
    }
    
    writer.finish().map_err(|e| format!("Failed to finalize EPUB archive: {}", e))?;
    Ok(output_path_str)
}

/// Build a batch translation prompt for multiple segments.
///
/// Generates a prompt that asks the LLM to translate N segments from source
/// to target language, wrapping each segment in `[SEG_N]` markers so the
/// response can be parsed programmatically.
pub fn build_batch_prompt(segments: &[(usize, &str)], source: &Language, target: &Language) -> String {
    let mut prompt = format!(
        "Translate the following {} segments from {} to {}.\n\
         Each segment is wrapped in [SEG_N] markers.\n\
         In your response, preserve the [SEG_N] markers exactly.\n\
         Place each translation immediately after its marker.\n\
         DO NOT translate code, variable names, or technical identifiers.\n",
        segments.len(),
        source.to_ietf_tag(),
        target.to_ietf_tag(),
    );

    for (idx, text) in segments {
        prompt.push_str(&format!("\n[SEG_{}]\n{}\n", idx, text));
    }

    prompt
}

/// Parse a batch LLM response into individual translations.
///
/// Uses regex to match `[SEG_N]` markers and their content.
/// Returns translations sorted by the original segment index.
/// Fails with an error message if the number of translations found
/// does not match `expected_count`.
pub fn parse_batch_response(response: &str, expected_count: usize) -> Result<Vec<String>, String> {
    let re = Regex::new(r"\[SEG_(\d+)\]")
        .map_err(|e| format!("Failed to compile batch response regex: {}", e))?;

    // Collect all marker positions and indices
    let mut markers: Vec<(usize, usize, usize)> = Vec::new(); // (index, start, end)
    for cap in re.captures_iter(response) {
        let m = cap.get(0).unwrap();
        let idx: usize = cap
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .ok_or_else(|| format!("Invalid segment index in response: {}", &cap[0]))?;
        markers.push((idx, m.start(), m.end()));
    }

    if markers.is_empty() {
        return Err(format!(
            "Expected {} translations in batch response, but found 0",
            expected_count
        ));
    }

    let mut entries: Vec<(usize, String)> = Vec::new();
    for (i, &(idx, _, end)) in markers.iter().enumerate() {
        let content_start = end;
        let content_end = if i + 1 < markers.len() {
            markers[i + 1].1
        } else {
            response.len()
        };
        let text = response[content_start..content_end].trim().to_string();
        entries.push((idx, text));
    }

    if entries.len() != expected_count {
        return Err(format!(
            "Expected {} translations in batch response, but found {}",
            expected_count,
            entries.len()
        ));
    }

    // Sort by original index to guarantee order
    entries.sort_by_key(|(idx, _)| *idx);

    Ok(entries.into_iter().map(|(_, text)| text).collect())
}

/// Build a retry prompt for a failed batch translation.
///
/// Prepends a critical instruction reminding the LLM to preserve all
/// `[SEG_N]` markers exactly.
pub fn build_retry_prompt(original_prompt: &str) -> String {
    format!(
        "CRITICAL: You must preserve ALL [SEG_N] markers exactly as shown in your response. \
         Do not skip, merge, or modify any markers.\n\n{}",
        original_prompt
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_happy_path() {
        let input = "[SEG_0]\n你好\n\n[SEG_1]\n世界\n\n[SEG_2]\n测试\n";
        let result = parse_batch_response(input, 3);
        let translations = result.expect("happy path should succeed");
        assert_eq!(translations.len(), 3);
        assert_eq!(translations[0], "你好");
        assert_eq!(translations[1], "世界");
        assert_eq!(translations[2], "测试");
    }

    #[test]
    fn test_parse_single_segment() {
        let input = "[SEG_0]\nhello\n";
        let result = parse_batch_response(input, 1);
        let translations = result.expect("single segment should succeed");
        assert_eq!(translations, vec!["hello"]);
    }

    #[test]
    fn test_parse_wrong_count() {
        let input = "[SEG_0]\nonly one\n";
        let result = parse_batch_response(input, 3);
        let err = result.expect_err("wrong count should fail");
        assert!(err.contains("Expected 3"), "error should mention Expected 3, got: {}", err);
    }

    #[test]
    fn test_parse_malformed() {
        let input = "no markers here at all";
        let result = parse_batch_response(input, 1);
        assert!(result.is_err(), "malformed input should fail");
    }

    #[test]
    fn test_parse_whitespace() {
        let input = "[SEG_0]  \n  bonjour  \n\n[SEG_1]\t\nmonde  \n";
        let result = parse_batch_response(input, 2);
        let translations = result.expect("whitespace input should succeed");
        assert_eq!(translations[0], "bonjour");
        assert_eq!(translations[1], "monde");
    }

    #[test]
    fn test_parse_empty_segment() {
        let input = "[SEG_0]\nhello\n\n[SEG_1]\n\n[SEG_2]\nworld\n";
        let result = parse_batch_response(input, 3);
        let translations = result.expect("empty segment should succeed");
        assert_eq!(translations[1], "");
    }
}
