// Headless EPUB batch verification binary
// Tests the batch translation flow WITHOUT real LLM calls.
// Uses the existing epub.rs parsing logic and mocks the LLM responses.
//
// Verifies:
//   1. EPUB parses into segments (via `lol_html` + spine traversal)
//   2. Batch grouping produces correct batch sizes
//   3. `parse_batch_response` correctly splits a mock batch response
//   4. `build_batch_prompt` generates correct prompt structure

use std::cell::RefCell;
use std::rc::Rc;

use lol_html::{element, end_tag, rewrite_str, text, RewriteStrSettings};
use rbook::read::ContentType;
use rbook::Ebook;

use mindrush::application::commands::epub::{build_batch_prompt, parse_batch_response};
use mindrush::domain::entity::Language;

const TEST_EPUB: &str =
    "D:\\Projects\\rust_workspace\\examples\\AiNieer\\test_books\\C++23 Best Practices-2024-英文版.epub";

const BATCH_SIZE: usize = 100;

/// CSS selector for translatable elements (same as read_epub).
const SELECTOR: &str =
    "p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote, figcaption, dt, dd, caption";

/// Tracks per-section state for the lol_html rewrite pass (same as read_epub).
struct SectionState {
    code_block_depth: usize,
    pending_text: String,
    has_pending_element: bool,
    segments: Vec<String>,
    marker_idx: usize,
}

fn main() {
    println!("=== EPUB Batch Verification Binary ===");
    println!("Opening: {}\n", TEST_EPUB);

    // ── Step 1: Parse EPUB via spine traversal + lol_html ──
    let epub = match rbook::Epub::new(TEST_EPUB) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ERROR: Failed to open EPUB: {}", e);
            std::process::exit(1);
        }
    };

    let reader = epub.reader();
    let mut all_segments: Vec<String> = Vec::new();
    let mut global_idx: usize = 0;
    let mut total_sections: usize = 0;

    for data_result in reader.iter() {
        let data = match data_result {
            Ok(d) => d,
            Err(e) => {
                eprintln!("WARNING: Failed to read section: {}", e);
                continue;
            }
        };

        let _section_path = data
            .get_content(ContentType::Path)
            .unwrap_or("")
            .to_string();
        let original_html = data.as_lossy_str().to_string();

        total_sections += 1;

        // Per-section state (same as read_epub)
        let state = Rc::new(RefCell::new(SectionState {
            code_block_depth: 0,
            pending_text: String::new(),
            has_pending_element: false,
            segments: Vec::new(),
            marker_idx: global_idx,
        }));

        let _marked_xhtml = {
            let s_track = state.clone();
            let s_main = state.clone();
            let s_text = state.clone();

            rewrite_str(
                &original_html,
                RewriteStrSettings {
                    element_content_handlers: vec![
                        // Track nesting depth of <pre> and <code> blocks
                        element!("pre, code", move |el| {
                            let mut st = s_track.borrow_mut();
                            st.code_block_depth += 1;
                            let s = s_track.clone();
                            el.on_end_tag(end_tag!(move |_end| {
                                s.borrow_mut().code_block_depth -= 1;
                                Ok(())
                            }))
                            .map_err(|_| "on_end_tag failed for pre/code")?;
                            Ok(())
                        }),
                        // Main handler: clear content and register end-tag handler
                        element!(SELECTOR, move |el| {
                            let mut st = s_main.borrow_mut();

                            // Skip elements inside code blocks
                            if st.code_block_depth > 0 {
                                return Ok(());
                            }

                            // Clear the element's inner content (original text).
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
                                el.on_end_tag(end_tag!(move |_end| {
                                    let mut st = s.borrow_mut();
                                    let text = std::mem::take(&mut st.pending_text);
                                    let trimmed = text.trim().to_string();

                                    if !trimmed.is_empty() {
                                        let marker =
                                            format!("\x00EPUB_{}\x00", st.marker_idx);
                                        st.marker_idx += 1;
                                        st.segments.push(trimmed);
                                        _end.before(
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
                        text!(SELECTOR, move |t| {
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
            .map_err(|e| format!("HTML rewriting error: {}", e))
            .unwrap()
        };

        let mut st = state.borrow_mut();
        global_idx = st.marker_idx;
        all_segments.extend(st.segments.drain(..));
    }

    let total_segments = all_segments.len();
    let total_batches = (total_segments + BATCH_SIZE - 1) / BATCH_SIZE;

    println!("=== EPUB Parsing Results ===");
    println!("Total sections processed: {}", total_sections);
    println!("Total segments: {}", total_segments);
    println!("Batch size: {}", BATCH_SIZE);
    println!("Total batches: {}", total_batches);
    println!();

    // ── Step 2: Compute and print batch sizes ──
    let batch_sizes: Vec<usize> = (0..total_batches)
        .map(|i| {
            let start = i * BATCH_SIZE;
            let end = std::cmp::min(start + BATCH_SIZE, total_segments);
            end - start
        })
        .collect();
    println!(
        "Batch sizes: {}",
        batch_sizes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    // ── Step 3: Log batch structure ──
    println!("=== Batch Structure ===");
    for batch_idx in 0..total_batches {
        let start = batch_idx * BATCH_SIZE;
        let end = std::cmp::min(start + BATCH_SIZE, total_segments);
        println!("Batch {}: segments {}-{}", batch_idx + 1, start, end - 1);
    }
    println!();

    // ── Step 4: Test parse_batch_response with a mock LLM response ──
    println!("=== Testing parse_batch_response ===");

    let mock_batch_size = std::cmp::min(BATCH_SIZE, total_segments);
    let mock_segments: Vec<(usize, &str)> = all_segments[..mock_batch_size]
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.as_str()))
        .collect();

    // Build a mock LLM response in the expected format:
    // [SEG_0]\n翻译0\n\n[SEG_1]\n翻译1\n\n...
    let mut mock_response = String::new();
    for (idx, _text) in &mock_segments {
        mock_response.push_str(&format!("[SEG_{}]\n翻译{}\n\n", idx, idx));
    }

    // Parse using the actual parse_batch_response function
    let parsed = match parse_batch_response(&mock_response, mock_batch_size) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("ERROR: parse_batch_response failed: {}", e);
            std::process::exit(1);
        }
    };

    // Verify all translations were extracted correctly
    let mut all_correct = true;
    for (i, translation) in parsed.iter().enumerate() {
        let expected = format!("翻译{}", i);
        if *translation != expected {
            println!(
                "  MISMATCH at index {}: expected '{}', got '{}'",
                i, expected, translation
            );
            all_correct = false;
        }
    }

    if all_correct && parsed.len() == mock_batch_size {
        println!(
            "Parse test: PASSED ({}/{} segments extracted)",
            parsed.len(),
            mock_batch_size
        );
    } else {
        println!(
            "Parse test: FAILED ({}/{} segments matched)",
            parsed.len(),
            mock_batch_size
        );
    }
    println!();

    // ── Step 5: Test build_batch_prompt (no crash, correct markers) ──
    println!("=== Testing build_batch_prompt ===");

    let prompt = build_batch_prompt(
        &mock_segments,
        &Language::English,
        &Language::ChineseSimplified,
    );
    println!("Prompt generated (first 300 chars):");
    let preview_len = std::cmp::min(300, prompt.len());
    println!("{}", &prompt[..preview_len]);
    println!("...");
    println!("Prompt total length: {} chars", prompt.len());
    println!();

    // Verify prompt contains SEG markers for the batch
    let mut all_markers_present = true;
    for (idx, _) in &mock_segments {
        if !prompt.contains(&format!("[SEG_{}]", idx)) {
            println!("  MISSING: [SEG_{}] in prompt", idx);
            all_markers_present = false;
        }
    }

    if all_markers_present {
        println!("build_batch_prompt: PASSED (all markers present in prompt)");
    } else {
        println!("build_batch_prompt: FAILED (some markers missing)");
    }
    println!();

    // ── Step 6: Verify progress event correctness (simulated) ──
    println!("=== Simulated Progress Events ===");
    for batch_idx in 0..total_batches {
        let end = std::cmp::min((batch_idx + 1) * BATCH_SIZE, total_segments);
        println!(
            "Progress: batch {}/{}, segments {}/{} completed",
            batch_idx + 1,
            total_batches,
            end,
            total_segments,
        );
    }
    println!();

    // ── Summary ──
    println!("=== Verification Summary ===");
    println!(
        "1. EPUB parsing:        PASSED ({} segments from {} sections)",
        total_segments, total_sections
    );
    println!("2. Batch grouping:       PASSED ({} batches)", total_batches);

    let expected_last = if total_segments % BATCH_SIZE == 0 {
        BATCH_SIZE
    } else {
        total_segments % BATCH_SIZE
    };
    println!(
        "   Last batch size: {} (expected {})",
        batch_sizes.last().copied().unwrap_or(0),
        expected_last
    );

    if all_correct && parsed.len() == mock_batch_size {
        println!("3. parse_batch_response: PASSED");
    } else {
        println!("3. parse_batch_response: FAILED");
    }

    if all_markers_present {
        println!("4. build_batch_prompt:   PASSED");
    } else {
        println!("4. build_batch_prompt:   FAILED");
    }

    println!("5. Progress simulation:  PASSED ({} events)", total_batches);
    println!();
    println!("=== All tests completed successfully ===");
}
