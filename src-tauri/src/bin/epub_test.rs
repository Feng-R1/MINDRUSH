// Headless EPUB reader test binary
// Tests EPUB reading logic without the Tauri GUI.
// Opens an EPUB, traverses spine sections via lol_html rewrite,
// counts segments, code blocks, and sections.

use std::cell::RefCell;
use std::rc::Rc;

use lol_html::{element, end_tag, rewrite_str, text, RewriteStrSettings};
use rbook::read::ContentType;
use rbook::Ebook;

const TEST_EPUB: &str =
    "D:\\Projects\\rust_workspace\\examples\\AiNieer\\test_books\\C++23 Best Practices-2024-英文版.epub";

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

/// Tracks aggregates across all sections.
struct Aggregates {
    total_segments: usize,
    total_sections: usize,
    total_code_blocks: usize,
    first_three_segments: Vec<String>,
    first_code_text_found: Option<String>,
}

fn main() {
    println!("=== EPUB Test Binary ===");
    println!("Opening: {}\n", TEST_EPUB);

    let epub = match rbook::Epub::new(TEST_EPUB) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ERROR: Failed to open EPUB: {}", e);
            std::process::exit(1);
        }
    };

    let reader = epub.reader();
    let mut agg = Aggregates {
        total_segments: 0,
        total_sections: 0,
        total_code_blocks: 0,
        first_three_segments: Vec::new(),
        first_code_text_found: None,
    };
    let mut global_idx: usize = 0;
    let mut first_code_found: bool = false;

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

        agg.total_sections += 1;

        // --- Per-section state (same as read_epub) ---
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
                        // so we can skip translatable elements inside them.
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
                        // for translatable elements (p, h1-h6, li, etc.)
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

        // Collect code block info before draining segments
        // Count code blocks by checking if the section had any nesting
        let section_code_depth = state.borrow().code_block_depth;

        let mut st = state.borrow_mut();
        global_idx = st.marker_idx;

        // Track code block content from the marked html (first one found)
        if !first_code_found {
            // Try to find any text inside code blocks by looking at the
            // original html for <code> or <pre> tags with content
            if let Some(code_text) = extract_first_code_text(&original_html) {
                agg.first_code_text_found = Some(code_text);
                first_code_found = true;
            }
        }

        // Track first three segments
        for seg in st.segments.drain(..) {
            if agg.first_three_segments.len() < 3 {
                agg.first_three_segments.push(
                    if seg.len() > 50 {
                        format!("{}...", &seg[..50])
                    } else {
                        seg.clone()
                    },
                );
            }
            agg.total_segments += 1;
        }

        // If code_block_depth was > 0 at end of section, something was nested
        if section_code_depth > 0 {
            agg.total_code_blocks += section_code_depth;
        }

        // Also count from the original_html for a more accurate code block count
        agg.total_code_blocks += count_code_blocks(&original_html);
    }

    // --- Output results ---
    println!("=== Results ===\n");
    println!("Total sections processed:  {}", agg.total_sections);
    println!("Total segments extracted:  {}", agg.total_segments);
    println!(
        "Total code blocks skipped: {}",
        agg.total_code_blocks
    );
    println!();

    println!("--- First 3 segments (truncated to 50 chars) ---");
    if agg.first_three_segments.is_empty() {
        println!("  (no segments found)");
    } else {
        for (i, seg) in agg.first_three_segments.iter().enumerate() {
            println!("  [{}] {}", i + 1, seg);
        }
    }
    println!();

    println!("--- First code block found (to confirm code blocks are skipped) ---");
    match &agg.first_code_text_found {
        Some(text) => {
            let truncated = if text.len() > 80 {
                format!("{}...", &text[..80])
            } else {
                text.clone()
            };
            println!("  \"{}\"", truncated);
            println!("  (This content should NOT appear in any segment)");
        }
        None => {
            println!("  (no code blocks found in this EPUB)");
        }
    }
    println!();

    println!("=== Test completed successfully ===");
}

/// Count <code> and <pre> tags in HTML content.
fn count_code_blocks(html: &str) -> usize {
    html.matches("<code").count() + html.matches("<pre").count()
}

/// Extract text from the first <code> or <pre> block found in HTML.
fn extract_first_code_text(html: &str) -> Option<String> {
    // Try <code> first
    for tag in &["<code", "<pre"] {
        if let Some(start) = html.find(tag) {
            // Find the closing >
            let gt = html[start..].find('>')?;
            let content_start = start + gt + 1;
            // Find closing tag
            let closing_tag = if *tag == "<code" { "</code>" } else { "</pre>" };
            if let Some(end) = html[content_start..].find(closing_tag) {
                let content = &html[content_start..content_start + end];
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}
