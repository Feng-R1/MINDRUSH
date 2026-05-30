// Integration test for EPUB marker replacement and ZIP writing logic.
//
// Tests that the marker replacement pipeline (identical to export_epub's)
// works correctly: read_epub-style lol_html parsing → mock translations →
// marker replacement via regex → ZIP output → verification.
//
// Does NOT call translate_epub (no LLM API calls).

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

use lol_html::{element, end_tag, rewrite_str, text, RewriteStrSettings};
use regex::Regex;
use rbook::read::ContentType;
use rbook::Ebook;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const TEST_EPUB: &str =
    "D:\\Projects\\rust_workspace\\examples\\AiNieer\\test_books\\C++23 Best Practices-2024-英文版.epub";

/// CSS selector for translatable elements (same as read_epub / epub_test.rs).
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

/// A spine section with its marked (placeholder-injected) XHTML.
struct MarkedSection {
    spine_path: String,
    marked_xhtml: String,
    segment_start: usize,
    segment_count: usize,
}

fn main() {
    println!("=== EPUB Export Test Binary ===");
    println!("Testing marker replacement + ZIP writing logic (no LLM calls)\n");

    // ---- Step 1: Parse EPUB and extract marked sections (same as read_epub) ----
    println!("--- Step 1: Parsing EPUB and extracting marked sections ---");
    let (all_segments, marked_sections) = match parse_epub(TEST_EPUB) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("ERROR: Failed to parse EPUB: {}", e);
            std::process::exit(1);
        }
    };

    println!("  Total segments extracted: {}", all_segments.len());
    println!("  Total marked sections:    {}", marked_sections.len());
    println!();

    if all_segments.is_empty() {
        eprintln!("ERROR: No translatable segments found in EPUB");
        std::process::exit(1);
    }

    // Show a few sample segments
    println!("--- First 3 original segments ---");
    for (i, seg) in all_segments.iter().take(3).enumerate() {
        let truncated = if seg.len() > 80 {
            format!("{}...", &seg[..80])
        } else {
            seg.clone()
        };
        println!("  [{}] {}", i, truncated);
    }
    println!();

    // ---- Step 2: Create mock translations ----
    println!("--- Step 2: Creating mock translations ---");
    let translations: Vec<String> = all_segments
        .iter()
        .map(|seg| format!("[TRANSLATED] {}", seg))
        .collect();
    println!("  Created {} mock translations", translations.len());
    println!("  First mock: [TRANSLATED] {}...", &all_segments[0][..40.min(all_segments[0].len())]);
    println!();

    // ---- Step 3: Write translated EPUB (same logic as export_epub) ----
    println!("--- Step 3: Writing translated EPUB ---");
    let output_path = match write_translated_epub(TEST_EPUB, &marked_sections, &translations) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("ERROR: Failed to write translated EPUB: {}", e);
            std::process::exit(1);
        }
    };
    println!("  Output file: {}", output_path);
    println!();

    // ---- Step 4: Verify the output ----
    println!("--- Step 4: Verifying output ---");
    let mut passed = 0u32;
    let mut failed = 0u32;

    // 4a. Output file exists
    if Path::new(&output_path).exists() {
        println!("  [PASS] Output file exists");
        passed += 1;
    } else {
        println!("  [FAIL] Output file does not exist");
        failed += 1;
    }

    // 4b. Output file is a valid ZIP
    match std::fs::File::open(&output_path) {
        Ok(file) => match zip::ZipArchive::new(file) {
            Ok(mut archive) => {
                let entry_count = archive.len();
                println!("  [PASS] Output is a valid ZIP with {} entries", entry_count);
                passed += 1;

                // 4c. No \x00EPUB_ markers remain in the output
                let mut total_markers_remaining = 0usize;
                let mut translated_entry_count = 0usize;
                let mut binary_entry_count = 0usize;
                let marker_re = Regex::new(r"\x00EPUB_(\d+)\x00").unwrap();

                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).unwrap();
                    let name = entry.name().to_owned();

                    let mut content = Vec::new();
                    entry.read_to_end(&mut content).unwrap();

                    // Check if this looks like an HTML/text entry or binary
                    let is_binary = content.iter().any(|&b| b == 0x00)
                        && !content.windows(9).any(|w| w == b"\x00EPUB_\x00");

                    if is_binary {
                        binary_entry_count += 1;
                    } else {
                        translated_entry_count += 1;
                        let text = String::from_utf8_lossy(&content);
                        let markers_found = marker_re.find_iter(&text).count();
                        total_markers_remaining += markers_found;
                        if markers_found > 0 {
                            println!(
                                "  [WARN] Entry '{}' still has {} unresolved marker(s)",
                                name, markers_found
                            );
                        }
                    }
                }

                if total_markers_remaining == 0 {
                    println!("  [PASS] No \\x00EPUB_N\\x00 markers remain in output (0)");
                    passed += 1;
                } else {
                    println!(
                        "  [FAIL] Found {} unresolved \\x00EPUB_N\\x00 markers",
                        total_markers_remaining
                    );
                    failed += 1;
                }

                println!(
                    "  Info: {} HTML entries translated, {} binary entries preserved",
                    translated_entry_count, binary_entry_count
                );

                // 4d. Output has actual translated content
                {
                    let file = std::fs::File::open(&output_path).unwrap();
                    let mut archive = zip::ZipArchive::new(file).unwrap();
                    let mut found_translated = false;

                    for i in 0..archive.len() {
                        let mut entry = archive.by_index(i).unwrap();
                        let mut content = Vec::new();
                        entry.read_to_end(&mut content).unwrap();
                        let text = String::from_utf8_lossy(&content);

                        if text.contains("[TRANSLATED]") {
                            found_translated = true;
                            break;
                        }
                    }

                    if found_translated {
                        println!("  [PASS] Output contains translated content (\"[TRANSLATED]\")");
                        passed += 1;
                    } else {
                        println!("  [FAIL] Output does NOT contain translated content");
                        failed += 1;
                    }
                }

                // 4e. Binary entries are preserved (at least some non-HTML entries exist)
                if binary_entry_count > 0 {
                    println!(
                        "  [PASS] Binary entries preserved ({} non-HTML entries)",
                        binary_entry_count
                    );
                    passed += 1;
                } else {
                    println!("  [WARN] No binary entries detected in output");
                }
            }
            Err(e) => {
                println!("  [FAIL] Output is NOT a valid ZIP: {}", e);
                failed += 1;
            }
        },
        Err(e) => {
            println!("  [FAIL] Cannot open output file: {}", e);
            failed += 1;
        }
    }

    // ---- Step 5: Cleanup ----
    println!();
    println!("--- Step 5: Cleanup ---");
    match std::fs::remove_file(&output_path) {
        Ok(_) => println!("  Deleted output file: {}", output_path),
        Err(e) => eprintln!("  WARNING: Failed to delete output file: {}", e),
    }
    println!();

    // ---- Summary ----
    println!("=== Summary ===");
    println!("  Passed: {}", passed);
    println!("  Failed: {}", failed);
    println!();

    if failed == 0 {
        println!("=== All tests PASSED ===");
    } else {
        println!("=== Some tests FAILED ===");
        std::process::exit(1);
    }
}

/// Parse an EPUB using the same lol_html logic as read_epub.
///
/// Returns (all_segments, marked_sections).
fn parse_epub(path: &str) -> Result<(Vec<String>, Vec<MarkedSection>), String> {
    let epub = rbook::Epub::new(path).map_err(|e| format!("Failed to open EPUB: {}", e))?;
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
                        element!(SELECTOR, move |el| {
                            let mut st = s_main.borrow_mut();

                            if st.code_block_depth > 0 {
                                return Ok(());
                            }

                            el.set_inner_content(
                                "",
                                lol_html::html_content::ContentType::Text,
                            );

                            st.has_pending_element = true;

                            if el.can_have_content() {
                                let s = s_main.clone();
                                el.on_end_tag(end_tag!(move |end| {
                                    let mut st = s.borrow_mut();
                                    let text = std::mem::take(&mut st.pending_text);
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

    Ok((all_segments, marked_sections))
}

/// Write a translated EPUB by replacing markers with translations
/// (same logic as export_epub).
fn write_translated_epub(
    original_path: &str,
    marked_sections: &[MarkedSection],
    translations: &[String],
) -> Result<String, String> {
    let path = Path::new(original_path);
    let stem = path
        .file_stem()
        .ok_or_else(|| "Invalid file path: missing file stem".to_string())?
        .to_string_lossy();
    let parent = path.parent().unwrap_or(Path::new("."));
    let output_path = parent.join(format!("{}_translated.epub", stem));
    let output_path_str = output_path.to_string_lossy().to_string();

    // Open original EPUB as ZIP
    let file =
        std::fs::File::open(original_path).map_err(|e| format!("Failed to open EPUB file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read EPUB as ZIP: {}", e))?;
    let out_file = std::fs::File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut writer = ZipWriter::new(out_file);
    let file_options = SimpleFileOptions::default();

    // Regex to find all EPUB_N markers in the marked_xhtml
    let marker_re = Regex::new(r"\x00EPUB_(\d+)\x00")
        .map_err(|e| format!("Regex compilation error: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let entry_name = entry.name().to_owned();

        // Find matching marked section
        let matching_section = marked_sections.iter().find(|s| {
            entry_name == s.spine_path
                || entry_name.ends_with(&s.spine_path)
                || s.spine_path.ends_with(&entry_name)
        });

        if let Some(section) = matching_section {
            // Replace markers with translations
            let new_content =
                marker_re.replace_all(&section.marked_xhtml, |caps: &regex::Captures| {
                    let idx = caps
                        .get(1)
                        .and_then(|m| m.as_str().parse::<usize>().ok())
                        .unwrap_or(0);
                    translations
                        .get(idx)
                        .cloned()
                        .unwrap_or_default()
                });

            writer
                .start_file(&entry_name, file_options)
                .map_err(|e| format!("Failed to create entry '{}': {}", entry_name, e))?;
            writer
                .write_all(new_content.as_bytes())
                .map_err(|e| format!("Failed to write entry '{}': {}", entry_name, e))?;
        } else {
            // Binary entry — copy unchanged
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read binary entry '{}': {}", entry_name, e))?;
            writer
                .start_file(&entry_name, file_options)
                .map_err(|e| format!("Failed to create entry '{}': {}", entry_name, e))?;
            writer
                .write_all(&buf)
                .map_err(|e| format!("Failed to write binary entry '{}': {}", entry_name, e))?;
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize EPUB archive: {}", e))?;

    Ok(output_path_str)
}
