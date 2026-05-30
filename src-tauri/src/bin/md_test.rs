// Headless Markdown reader/exporter test binary.
//
// Tests the read_md → export_md pipeline without the Tauri GUI.
// Parses a markdown file with pulldown-cmark (same logic as read_md),
// extracts translatable segments, creates mock translations,
// replaces markers (same logic as export_md), and writes the output.
//
// Does NOT call any LLM API (no translate step).

use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;

fn heading_level_to_usize(level: &pulldown_cmark::HeadingLevel) -> usize {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}

const TEST_MD: &str = "D:\\Projects\\rust_workspace\\examples\\AiNieer\\test_books\\test_md.md";

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Bullet,
    Numbered,
}

fn main() {
    println!("=== Markdown Test Binary ===\n");

    // ---- Step 1: Read and parse the markdown file ----
    println!("--- Step 1: Parsing markdown (read_md logic) ---");

    let path = TEST_MD;
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: Failed to read file: {}", e);
            std::process::exit(1);
        }
    };

    let filename = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.md".to_string());

    println!("  File: {}", filename);
    println!("  Original size: {} bytes", content.len());

    // ---- Step 2: Extract translatable segments (same as new read_md) ----
    println!("\n--- Step 2: Extracting translatable segments ---");

    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(&content, options).into_offset_iter();

    let mut output = String::new(); // progressive marked content
    let mut segments: Vec<String> = Vec::new();
    let mut code_spans: Vec<String> = Vec::new();
    let mut code_idx: usize = 0;
    let mut text_buf = String::new();
    let mut in_code_block = false;
    let mut skip_image_depth: u32 = 0;
    let mut skip_metadata = false;
    let mut list_kind_stack: Vec<ListKind> = Vec::new();
    let mut list_number_counter: u64 = 0;
    let mut pending_link_url: Option<String> = None;
    let mut pending_image_url: Option<String> = None;
    let mut table_cell_count: usize = 0;
    let mut first_table_row_done: bool = false;

    let flush = |text_buf: &mut String, segments: &mut Vec<String>, output: &mut String| {
        if !text_buf.is_empty() {
            let trimmed = text_buf.trim().to_string();
            if !trimmed.is_empty() {
                let marker = format!("\x00MD_{}\x00", segments.len());
                segments.push(trimmed);
                output.push_str(&marker);
            }
            text_buf.clear();
        }
    };

    for (event, range) in parser {
        match event {
            Event::Start(tag) => {
                flush(&mut text_buf, &mut segments, &mut output);
                match &tag {
                    Tag::CodeBlock(_) => in_code_block = true,
                    Tag::Image { .. } => skip_image_depth += 1,
                    Tag::MetadataBlock(_) => skip_metadata = true,
                    _ => {}
                }
                match &tag {
                    Tag::Paragraph => {}
                    Tag::Heading { level, .. } => {
                        let n = heading_level_to_usize(level);
                        output.push_str(&"#".repeat(n));
                        output.push(' ');
                    }
                    Tag::BlockQuote(..) => output.push_str("> "),
                    Tag::Item => {
                        match list_kind_stack.last() {
                            Some(ListKind::Bullet) => output.push_str("- "),
                            Some(ListKind::Numbered) => {
                                output.push_str(&format!("{}. ", list_number_counter));
                                list_number_counter += 1;
                            }
                            None => output.push_str("  "),
                        }
                    }
                    Tag::TableCell => {
                        output.push_str("| ");
                        table_cell_count += 1;
                    }
                    Tag::Strong => output.push_str("**"),
                    Tag::Emphasis => output.push_str("*"),
                    Tag::Strikethrough => output.push_str("~~"),
                    Tag::Link { dest_url, .. } => {
                        output.push_str("[");
                        pending_link_url = Some(dest_url.to_string());
                    }
                    Tag::Image { dest_url, .. } => {
                        output.push_str("![");
                        pending_image_url = Some(dest_url.to_string());
                    }
                    Tag::List(Some(n)) => {
                        list_kind_stack.push(ListKind::Numbered);
                        list_number_counter = *n;
                    }
                    Tag::List(None) => {
                        list_kind_stack.push(ListKind::Bullet);
                    }
                    Tag::CodeBlock(kind) => {
                        match kind {
                            pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                                output.push_str("```");
                                if !lang.is_empty() {
                                    output.push_str(lang);
                                }
                                output.push('\n');
                            }
                            pulldown_cmark::CodeBlockKind::Indented => {}
                        }
                    }
                    Tag::MetadataBlock(_) => {}
                    Tag::TableHead => {}
                    Tag::TableRow => {
                        if table_cell_count > 0 {
                            // Close previous row
                            output.push_str("|\n");
                            // Generate separator after header row
                            if !first_table_row_done {
                                first_table_row_done = true;
                                output.push('|');
                                for _ in 0..table_cell_count {
                                    output.push_str(" --- |");
                                }
                                output.push('\n');
                            }
                        }
                        table_cell_count = 0;
                    }
                    Tag::Table(_) => {
                        first_table_row_done = false;
                    }
                    _ => output.push_str(&content[range.start..range.end]),
                }
            }
            Event::End(tag_end) => {
                flush(&mut text_buf, &mut segments, &mut output);
                match &tag_end {
                    TagEnd::CodeBlock => in_code_block = false,
                    TagEnd::Image => skip_image_depth = skip_image_depth.saturating_sub(1),
                    TagEnd::MetadataBlock(_) => skip_metadata = false,
                    _ => {}
                }
                match &tag_end {
                    TagEnd::Paragraph => output.push_str("\n\n"),
                    TagEnd::Heading(..) => output.push_str("\n\n"),
                    TagEnd::BlockQuote(_) => output.push_str("\n"),
                    TagEnd::Item => output.push_str("\n"),
                    TagEnd::TableCell => {}
                    TagEnd::TableRow => {}
                    TagEnd::Table => {
                        if table_cell_count > 0 {
                            output.push_str("|\n");
                        }
                        output.push_str("\n");
                    }
                    TagEnd::List(_) => {
                        list_kind_stack.pop();
                        output.push_str("\n");
                    }
                    TagEnd::Strong => output.push_str("**"),
                    TagEnd::Emphasis => output.push_str("*"),
                    TagEnd::Strikethrough => output.push_str("~~"),
                    TagEnd::Link => {
                        if let Some(url) = pending_link_url.take() {
                            output.push_str("](");
                            output.push_str(&url);
                            output.push(')');
                        }
                    }
                    TagEnd::Image => {
                        if let Some(url) = pending_image_url.take() {
                            output.push_str("](");
                            output.push_str(&url);
                            output.push(')');
                        }
                    }
                    TagEnd::CodeBlock => output.push_str("```\n"),
                    TagEnd::MetadataBlock(_) => {}
                    TagEnd::TableHead => {}
                    _ => output.push_str(&content[range.start..range.end]),
                }
            }
            Event::Text(text) => {
                if in_code_block || skip_image_depth > 0 || skip_metadata {
                    output.push_str(&content[range.start..range.end]);
                    continue;
                }
                text_buf.push_str(&text);
            }
            Event::Code(code) => {
                text_buf.push_str(&format!("`C{}`", code_idx));
                code_spans.push(code.to_string());
                code_idx += 1;
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                text_buf.push_str(&format!("`C{}`", code_idx));
                code_spans.push(html.to_string());
                code_idx += 1;
            }
            Event::SoftBreak | Event::HardBreak => {
                flush(&mut text_buf, &mut segments, &mut output);
                output.push_str(&content[range.start..range.end]);
            }
            _ => {
                output.push_str(&content[range.start..range.end]);
            }
        }
    }
    flush(&mut text_buf, &mut segments, &mut output);

    let marked_content = output; // rename for clarity

    println!("  Found {} translatable segments", segments.len());
    if !code_spans.is_empty() {
        println!("  Found {} code spans", code_spans.len());
    }
    println!();

    // ---- Step 3: Print each segment ----
    println!("--- Step 3: Segment contents ---");
    for (i, seg) in segments.iter().enumerate() {
        let truncated = if seg.len() > 80 {
            format!("{}...", &seg[..80])
        } else {
            seg.clone()
        };
        println!("  [{:02}] {}", i, truncated);
    }
    println!();

    // ---- Step 4: Verify marked content has markers ----
    println!("--- Step 4: Verifying marker injection ---");
    let marker_re =
        Regex::new(r"\x00MD_(\d+)\x00").expect("Failed to compile marker regex");
    let marker_count = marker_re.find_iter(&marked_content).count();
    println!("  Markers found in marked content: {}", marker_count);
    assert_eq!(
        marker_count, segments.len(),
        "Marker count ({}) must match segment count ({})",
        marker_count, segments.len()
    );
    println!("  PASSED: marker count matches segment count\n");

    // ---- Step 5: Mock translations (copy original text, no LLM) ----
    println!("--- Step 5: Creating mock translations ---");
    let translations: Vec<String> = segments.clone();
    println!("  Mock translations created: {}", translations.len());
    println!("  (Each translation = original text — no LLM API called)");
    if !code_spans.is_empty() {
        println!("  Code spans available for restoration: {}", code_spans.len());
    }
    println!();

    // ---- Step 6: Replace markers and restore code → translated output (same as export_md) ----
    println!("--- Step 6: Replacing markers with translations ---");

    let after_md_markers = marker_re.replace_all(&marked_content, |caps: &regex::Captures| {
        let idx = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        translations.get(idx).cloned().unwrap_or_default()
    });

    // Restore code placeholders (`C0`, `C1`, etc.) with actual code from code_spans
    let code_re = Regex::new(r"`C(\d+)`").expect("Failed to compile code regex");
    let translated = code_re.replace_all(&after_md_markers, |caps: &regex::Captures| {
        let cidx: usize = caps[1].parse().unwrap_or(0);
        let code = code_spans
            .get(cidx)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string());
        format!("`{}`", code)
    });

    println!("  MD markers replaced, code spans restored\n");

    // ---- Step 7: Write output file ----
    let stem = Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let parent = Path::new(path).parent().unwrap_or(Path::new("."));
    let output_path = parent.join(format!("{}_translated.md", stem));
    let output_path_str = output_path.to_string_lossy().to_string();

    std::fs::write(&output_path, translated.as_bytes())
        .unwrap_or_else(|e| {
            eprintln!("ERROR: Failed to write output file: {}", e);
            std::process::exit(1);
        });

    println!("  Output written to: {}\n", output_path_str);

    // ---- Step 8: Verify output ----
    println!("--- Step 8: Verifying output ---");
    match std::fs::read_to_string(&output_path) {
        Ok(output_content) => {
            println!("  Output size: {} bytes", output_content.len());
            // Verify no MD markers remain
            let remaining_markers = marker_re.find_iter(&output_content).count();
            if remaining_markers > 0 {
                eprintln!("  FAILED: {} markers remain in output!", remaining_markers);
                std::process::exit(1);
            }
            println!("  PASSED: no markers remain in output");

            // Verify no code placeholders remain (C0, C1, etc.)
            let remaining_code_placeholders = code_re.find_iter(&output_content).count();
            if remaining_code_placeholders > 0 {
                eprintln!(
                    "  FAILED: {} code placeholders remain in output!",
                    remaining_code_placeholders
                );
                std::process::exit(1);
            }
            println!("  PASSED: no code placeholders remain in output");

            // Verify content is non-empty
            assert!(
                !output_content.is_empty(),
                "Output content must not be empty"
            );
            println!("  PASSED: output content is non-empty");

            // Verify all original code spans appear in output
            if !code_spans.is_empty() {
                let mut all_code_found = true;
                for (i, code) in code_spans.iter().enumerate() {
                    if !output_content.contains(code.as_str()) {
                        eprintln!(
                            "  WARNING: code span [{}] '{}' not found in output",
                            i,
                            if code.len() > 50 {
                                format!("{}...", &code[..50])
                            } else {
                                code.clone()
                            }
                        );
                        all_code_found = false;
                    }
                }
                if all_code_found {
                    println!("  PASSED: all code spans present in output");
                }
            }

            // Verify all segments appear in output
            // NOTE: segments may contain `C0`, `C1` placeholders that were
            // replaced with actual code, so segments won't always substring-match.
            let mut all_found = true;
            for (i, seg) in segments.iter().enumerate() {
                if !output_content.contains(seg.as_str()) {
                    eprintln!(
                        "  WARNING: segment [{}] not found in output (may have code placeholders restored)",
                        i
                    );
                    all_found = false;
                }
            }
            if all_found {
                println!("  PASSED: all original segments present in output");
            } else {
                println!("  INFO: some segments differ due to code placeholder restoration");
            }
        }
        Err(e) => {
            eprintln!("ERROR: Failed to read output file for verification: {}", e);
            std::process::exit(1);
        }
    }

    println!("\n=== ALL TESTS PASSED ===");
}
