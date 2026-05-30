// Application: Markdown commands

use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use tauri::{command, State};

use crate::application::state::{AppState, EpubDocument, EpubLoadInfo, MarkedSection, SegmentMeta, SegmentType};

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

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Bullet,
    Numbered,
}

/// Parse markdown content into an EpubDocument.
///
/// Walks pulldown-cmark events with byte offsets, collects translatable
/// text from non-code, non-frontmatter, non-image, non-HTML content
/// into segments, and inserts `\x00MD_N\x00` markers in place of the
/// original text.
pub fn parse_markdown_to_document(
    content: &str,
    filename: &str,
) -> Result<(EpubDocument, EpubLoadInfo), String> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(&content, options).into_offset_iter();

    // --- State ---
    let mut output = String::new(); // progressive marked_xhtml builder
    let mut segments: Vec<String> = Vec::new();
    let mut code_spans: Vec<String> = Vec::new();
    let mut code_idx: usize = 0;
    let mut text_buf = String::new(); // accumulates current paragraph's translatable text
    let mut in_code_block = false;
    let mut skip_image_depth: u32 = 0;
    let mut skip_metadata = false;
    let mut list_kind_stack: Vec<ListKind> = Vec::new();
    let mut list_number_counter: u64 = 0;
    let mut pending_link_url: Option<String> = None;
    let mut pending_image_url: Option<String> = None;
    let mut table_cell_count: usize = 0;
    let mut first_table_row_done: bool = false;

    // Flush accumulated text as one segment, emit marker to output
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

    let total_segments = segments.len();
    let doc = EpubDocument {
        original_path: String::new(), // caller sets this
        filename: filename.to_string(),
        segments,
        translations: vec![None; total_segments],
        segment_meta: vec![SegmentMeta {
            segment_type: SegmentType::Paragraph,
            keep_original: false,
            font_size: None,
            is_monospace: false,
            indent: None,
            is_italic: false,
        }; total_segments],
        marked_sections: vec![MarkedSection {
            spine_path: "root".to_string(),
            marked_xhtml: output,
            segment_start: 0,
            segment_count: total_segments,
        }],
        code_spans,
    };
    let load_info = EpubLoadInfo {
        filename: filename.to_string(),
        segment_count: total_segments,
    };

    Ok((doc, load_info))
}

/// Parse a markdown file and extract translatable text.
///
/// Walks pulldown-cmark events with byte offsets, collects translatable
/// text from non-code, non-frontmatter, non-image, non-HTML content
/// into segments, and inserts `\x00MD_N\x00` markers in place of the
/// original text. Stores the resulting EpubDocument in AppState.
#[command]
pub async fn read_md(
    state: State<'_, AppState>,
    path: String,
) -> Result<EpubLoadInfo, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let filename = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.md".to_string());

    let (mut doc, load_info) = parse_markdown_to_document(&content, &filename)?;
    doc.original_path = path.clone();
    *state.epub_document.write().await = Some(doc);

    Ok(load_info)
}

/// Export a translated markdown file.
///
/// Reads the stored EpubDocument from AppState, replaces every
/// `\x00MD_N\x00` marker with its corresponding translation,
/// and writes the result to `{filename}_translated.md` next to
/// the original file.
#[command]
pub async fn export_md(state: State<'_, AppState>) -> Result<String, String> {
    // Get document from state
    let (original_path, marked_sections, translations, segments, code_spans) = {
        let doc = state.epub_document.read().await;
        let doc = doc
            .as_ref()
            .ok_or("No document loaded. Please load and translate a markdown file first.")?;

        // Check all translations are complete
        let incomplete = doc
            .translations
            .iter()
            .enumerate()
            .find(|(_, t)| t.is_none());
        if let Some((i, _)) = incomplete {
            return Err(format!(
                "Translation incomplete: segment {} is not translated. \
                 Click 'Translate File' to retry remaining segments.",
                i
            ));
        }

        (
            doc.original_path.clone(),
            doc.marked_sections.clone(),
            doc.translations.clone(),
            doc.segments.clone(),
            doc.code_spans.clone(),
        )
    };

    let path = Path::new(&original_path);
    let stem = path
        .file_stem()
        .ok_or_else(|| "Invalid file path: missing file stem".to_string())?
        .to_string_lossy();
    let parent = path.parent().unwrap_or(Path::new("."));
    let output_path = parent.join(format!("{}_translated.md", stem));
    let output_path_str = output_path.to_string_lossy().to_string();

    // Regex to find all MD_N markers in the marked markdown
    let marker_re =
        Regex::new(r"\x00MD_(\d+)\x00").map_err(|e| format!("Regex compilation error: {}", e))?;

    // There should be exactly one marked section for markdown
    let section = marked_sections
        .first()
        .ok_or_else(|| "No marked content found in document".to_string())?;

    let after_md_markers = marker_re.replace_all(&section.marked_xhtml, |caps: &regex::Captures| {
        let idx = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        let translation = translations.get(idx).cloned().flatten().unwrap_or_default();
        let original = segments.get(idx).map(|s| s.as_str()).unwrap_or("");
        clean_segment_translation(&translation, original)
    });

    // Step 2: Restore code placeholders `C0`, `C1`, etc. with actual code
    let code_re = Regex::new(r"`C(\d+)`")
        .map_err(|e| format!("Regex error: {}", e))?;
    let final_output = code_re.replace_all(&after_md_markers, |caps: &regex::Captures| {
        let cidx: usize = caps[1].parse().unwrap_or(0);
        let code = code_spans.get(cidx).cloned().unwrap_or_else(|| caps[0].to_string());
        format!("`{}`", code)
    });

    std::fs::write(&output_path, final_output.as_bytes())
        .map_err(|e| format!("Failed to write output file: {}", e))?;

    Ok(output_path_str)
}

/// Cleans translation text by removing echoed original prefix.
///
/// When the LLM receives very short text fragments (e.g., "Ensure ", "The "),
/// it sometimes echoes back "Ensure -> 确保" instead of just "确保".
/// This strips the "original -> " prefix when detected.
fn clean_segment_translation(translation: &str, original: &str) -> String {
    if original.is_empty() || translation.is_empty() {
        return translation.to_string();
    }

    // Pattern 1: "original -> translated" with space separator
    let prefix = format!("{} -> ", original);
    if translation.starts_with(&prefix) {
        return translation[prefix.len()..].to_string();
    }

    // Pattern 2: "original ->translated" (no space after ->)
    let prefix2 = format!("{} ->", original);
    if translation.starts_with(&prefix2) {
        return translation[prefix2.len()..].trim_start().to_string();
    }

    // Pattern 3: "original   translated" (original just prepended, no ->)
    if translation.starts_with(original) && translation.len() > original.len() {
        let rest = &translation[original.len()..];
        let rest = rest.trim_start_matches(|c: char| c == ' ' || c == '-' || c == '>');
        if !rest.is_empty() && rest.len() < translation.len() {
            return rest.trim().to_string();
        }
    }

    translation.to_string()
}
