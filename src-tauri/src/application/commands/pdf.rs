// Application: PDF commands

use pdf_oxide::PdfDocument;
use pdf_oxide::extractors::SpanMergingConfig;
use pdf_oxide::layout::TextSpan;
use pdf_oxide::writer::{DocumentBuilder, EmbeddedFont};
use std::path::Path;
use tauri::{State, command};

use crate::application::state::{AppState, EpubDocument, EpubLoadInfo, MarkedSection, PdfDocumentData, PdfElement, SegmentMeta, SegmentType};

/// Group text spans into logical paragraphs with structure detection.
///
/// Detects:
/// - Code blocks: `is_monospace == true` → `SegmentType::Code`, `keep_original = true`
/// - Headings: `font_size > 14.0` → `SegmentType::Heading { level }`
/// - Regular paragraphs: default
/// Compute paragraph-break threshold from y-gaps on a single page.
fn compute_para_threshold(spans: &[TextSpan], boundaries: &[usize]) -> f32 {
    use std::collections::HashMap;
    let mut freq: HashMap<i32, u32> = HashMap::new();

    for w in boundaries.windows(2) {
        let start = w[0];
        let end = w[1].min(spans.len());
        if end <= start + 1 { continue; }
        for j in (start + 1)..end {
            let prev = &spans[j - 1];
            let curr = &spans[j];
            if prev.text.trim().is_empty() || curr.text.trim().is_empty() { continue; }
            let gap = prev.bbox.y - curr.bbox.y;
            if gap > 1.0 && gap < 200.0 {
                let key = (gap * 10.0).round() as i32;
                *freq.entry(key).or_default() += 1;
            }
        }
    }

    if freq.is_empty() { return 25.0_f32; }
    let mut sorted: Vec<(i32, u32)> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    let line = sorted.first().map(|(k, _)| *k as f32 / 10.0).unwrap_or(21.0);
    let para = sorted.get(1).map(|(k, _)| *k as f32 / 10.0).unwrap_or(line * 1.8);
    ((line + para) / 2.0).max(18.0)
}

/// Group text spans from a SINGLE page into logical segments.
fn group_spans_for_page(spans: &[TextSpan], para_threshold: f32) -> Vec<(String, SegmentMeta)> {
    if spans.is_empty() { return vec![]; }
    struct Group {
        text: String,
        max_font_size: f32,
        is_monospace: bool,
        is_italic: bool,
        min_x: f32,
    }

    let mut groups: Vec<Group> = Vec::new();
    let mut current = Group {
        text: String::new(),
        max_font_size: 0.0,
        is_monospace: false,
        is_italic: false,
        min_x: 9999.0,
    };
    let mut current_y: Option<f32> = None;

    for span in spans {
        if span.text.trim().is_empty() {
            continue;
        }

        let should_flush = match current_y {
            None => false,
            Some(prev_y) => {
                let font_jump = (span.font_size as f32 - current.max_font_size).abs();
                if span.bbox.y > prev_y {
                    // Page transition: y jumped from bottom (low) to top (high).
                    let gap = span.bbox.y - prev_y;
                    // Gap > page height means section/chapter break — always split.
                    if gap > 1000.0 {
                        true
                    } else {
                        // Normal page transition — split only if paragraph ended.
                        let trimmed = current.text.trim();
                        trimmed.ends_with('.') || trimmed.ends_with('!')
                            || trimmed.ends_with('?') || trimmed.ends_with('。')
                            || trimmed.ends_with('！') || trimmed.ends_with('？')
                    }
                } else {
                    // Normal flow (same page): y decreases as we read down
                    let gap = prev_y - span.bbox.y;
                    gap > para_threshold
                        || (font_jump > 4.0 && !current.text.trim().is_empty())
                }
            }
        };

        if should_flush {
            let trimmed = std::mem::take(&mut current.text);
            let trimmed = trimmed.trim().to_string();
            if !trimmed.is_empty() {
                groups.push(Group {
                    text: trimmed,
                    max_font_size: current.max_font_size,
                    is_monospace: current.is_monospace,
                    is_italic: current.is_italic,
                    min_x: current.min_x,
                });
            }
            current = Group {
                text: String::new(),
                max_font_size: 0.0,
                is_monospace: false,
                is_italic: false,
                min_x: 9999.0,
            };
        } else if current_y.is_some() && !span.is_monospace {
            // Merge consecutive lines with a space separator
            current.text.push(' ');
        }

        current.text.push_str(&span.text);
        current.min_x = current.min_x.min(span.bbox.x);
        current.max_font_size = current.max_font_size.max(span.font_size as f32);
        current.is_monospace = current.is_monospace || span.is_monospace;
        current.is_italic = current.is_italic || span.is_italic;
        current_y = Some(span.bbox.y);
    }

    // Flush final group
    let trimmed = current.text.trim().to_string();
    if !trimmed.is_empty() {
        groups.push(Group {
            text: trimmed,
            max_font_size: current.max_font_size,
            is_monospace: current.is_monospace,
            is_italic: current.is_italic,
            min_x: current.min_x,
        });
    }

    // Step 2: Classify each group into a segment
    groups
        .into_iter()
        .map(|g| {
            let meta = if g.is_monospace {
                SegmentMeta {
                    segment_type: SegmentType::Code,
                    keep_original: true,
                    font_size: Some(g.max_font_size),
                    is_monospace: true,
                    indent: if g.min_x < 9000.0 { Some(g.min_x - 72.0) } else { None },
                    is_italic: false,
                }
            } else if g.max_font_size > 14.0 {
                let level = if g.max_font_size > 18.0 {
                    1
                } else if g.max_font_size > 16.0 {
                    2
                } else {
                    3
                };
                SegmentMeta {
                    segment_type: SegmentType::Heading { level },
                    keep_original: false,
                    font_size: Some(g.max_font_size),
                    is_monospace: false,
                    indent: if g.min_x < 9000.0 { Some(g.min_x - 72.0) } else { None },
                    is_italic: false,
                }
            } else {
                SegmentMeta {
                    segment_type: SegmentType::Paragraph,
                    keep_original: false,
                    font_size: Some(g.max_font_size),
                    is_monospace: false,
                    indent: if g.min_x < 9000.0 { Some(g.min_x - 72.0) } else { None },
                    is_italic: false,
                }
            };
            (g.text, meta)
        })
        .collect()
}

/// Read a PDF file and extract translatable text via span-based extraction.
#[command]
pub async fn read_pdf(
    state: State<'_, AppState>,
    path: String,
) -> Result<EpubLoadInfo, String> {
    let doc = PdfDocument::open(&path)
        .map_err(|e| format!("Failed to open PDF: {}", e))?;

    let page_count = doc
        .page_count()
        .map_err(|e| format!("Failed to get page count: {}", e))?;

    // --- Pass 1: collect all spans with page boundaries ---
    let mut all_spans: Vec<TextSpan> = Vec::new();
    let mut boundaries: Vec<usize> = vec![0];
    for i in 0..page_count {
        let config = SpanMergingConfig::adaptive();
        let spans = doc
            .extract_spans_with_config(i, config)
            .map_err(|e| format!("Failed to extract spans from page {}: {}", i, e))?;
        all_spans.extend(spans);
        boundaries.push(all_spans.len());
    }

    // --- Compute document-level paragraph threshold (within-page gaps only) ---
    let para_threshold = compute_para_threshold(&all_spans, &boundaries);

    // --- Pass 2: per-page processing — text, code, images in order ---
    let mut elements: Vec<PdfElement> = Vec::new();
    let mut translatable_segments: Vec<String> = Vec::new();

    for (page_idx, w) in boundaries.windows(2).enumerate() {
        let page_spans = &all_spans[w[0]..w[1]];

        // Sort by y-coordinate for natural reading order
        let mut sorted: Vec<&TextSpan> = page_spans.iter().collect();
        sorted.sort_by(|a, b| b.bbox.y.partial_cmp(&a.bbox.y).unwrap());

        // Walk in y-order, flushing on text↔code boundaries
        let mut text_buf: Vec<TextSpan> = Vec::new();
        let mut code_lines: Vec<String> = Vec::new();
        let mut code_font: f32 = 11.0;
        let mut in_code = false;
        let mut start_y: f32 = 0.0;
        let mut page_elems: Vec<(f32, PdfElement)> = Vec::new();

        for span in &sorted {
            if span.text.trim().is_empty() { continue; }
            if span.is_monospace {
                if !in_code && !text_buf.is_empty() {
                    for (text, meta) in group_spans_for_page(&text_buf, para_threshold) {
                        if text.is_empty() { continue; }
                        let idx = translatable_segments.len();
                        translatable_segments.push(text.clone());
                        page_elems.push((start_y, PdfElement::TextSegment { segment_idx: idx, text, meta }));
                    }
                    text_buf.clear();
                }
                in_code = true;
                if code_lines.is_empty() { start_y = span.bbox.y; }
                code_lines.push(span.text.clone());
                code_font = span.font_size as f32;
            } else {
                if in_code && !code_lines.is_empty() {
                    page_elems.push((start_y, PdfElement::CodeBlock { text: code_lines.join("\n"), font_size: code_font }));
                    code_lines.clear();
                }
                in_code = false;
                if text_buf.is_empty() { start_y = span.bbox.y; }
                text_buf.push((*span).clone());
            }
        }
        // Flush remaining
        if in_code && !code_lines.is_empty() {
            page_elems.push((start_y, PdfElement::CodeBlock { text: code_lines.join("\n"), font_size: code_font }));
        } else if !text_buf.is_empty() {
            for (text, meta) in group_spans_for_page(&text_buf, para_threshold) {
                if text.is_empty() { continue; }
                let idx = translatable_segments.len();
                translatable_segments.push(text.clone());
                page_elems.push((start_y, PdfElement::TextSegment { segment_idx: idx, text, meta }));
            }
        }

        // --- Insert images at their y-position ---
        if let Ok(images) = doc.extract_images(page_idx) {
            for img in &images {
                if let Ok(png) = img.to_png_bytes() {
                    let img_y = img.bbox().map(|r| r.y).unwrap_or(0.0);
                    page_elems.push((img_y, PdfElement::Image {
                        data: png, x: 0.0_f32, y: 0.0_f32,
                        width: img.width() as f32, height: img.height() as f32,
                        page: page_idx, caption: None,
                    }));
                }
            }
        }

        // Sort by y desc (reading order) and append
        page_elems.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        // Ensure figure captions render after their images
        let mut i = 0;
        while i < page_elems.len() {
            let cap_y = page_elems[i].0;
            let is_caption = match &page_elems[i].1 {
                PdfElement::TextSegment { text, .. } => {
                    text.trim().starts_with("图") || text.contains("Figure")
                }
                _ => false,
            };
            if is_caption {
                // Find nearest image above this caption
                let mut nearest_img: Option<usize> = None;
                let mut nearest_dist = f32::MAX;
                for j in 0..page_elems.len() {
                    if matches!(page_elems[j].1, PdfElement::Image {..}) {
                        let dist = (page_elems[j].0 - cap_y).abs();
                        if dist < nearest_dist && dist < 100.0 {
                            nearest_dist = dist;
                            nearest_img = Some(j);
                        }
                    }
                }
                // If image found and caption is before it, move caption after
                if let Some(j) = nearest_img {
                    if i < j {
                        let elem = page_elems.remove(i);
                        page_elems.insert(j, elem);
                        i -= 1; // re-check this position
                    }
                }
            }
            i += 1;
        }
        for (_, elem) in page_elems { elements.push(elem); }

    }

    let filename = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.pdf".to_string());

    let total_segments = translatable_segments.len();

    let document = PdfDocumentData {
        original_path: path.clone(),
        filename: filename.clone(),
        elements,
        translatable_segments,
        translations: vec![None; total_segments],
    };

    *state.pdf_document.write().await = Some(document);

    Ok(EpubLoadInfo {
        filename,
        segment_count: total_segments,
    })
}

/// Export translated content as a real PDF.
///
/// Renders directly from segments, translations, and segment_meta —
/// no markdown intermediate representation.
#[command]
pub async fn export_pdf(state: State<'_, AppState>) -> Result<String, String> {
    let pdf_doc = {
        let doc = state.pdf_document.read().await;
        let doc = doc.as_ref().ok_or("No PDF loaded.")?;
        let incomplete = doc.translations.iter().enumerate().find(|(_, t)| t.is_none());
        if let Some((i, _)) = incomplete {
            return Err(format!("Translation incomplete: segment {}.", i));
        }
        doc.clone()
    };

    let path = Path::new(&pdf_doc.original_path);
    let stem = path.file_stem().ok_or("Invalid path")?.to_string_lossy();
    let parent = path.parent().unwrap_or(Path::new("."));
    let output_path = parent.join(format!("{}_translated.pdf", stem));

    let pdf_bytes = render_elements_to_pdf(&pdf_doc)?;
    std::fs::write(&output_path, &pdf_bytes)
        .map_err(|e| format!("Failed to write PDF: {}", e))?;

    Ok(output_path.to_string_lossy().to_string())
}

/// Translate all translatable segments in the loaded PDF by
/// reusing the existing EPUB translation pipeline.
#[command]
pub async fn translate_pdf(
    state: State<'_, AppState>,
    source_lang: String,
    target_lang: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let segments = {
        let doc = state.pdf_document.read().await;
        let doc = doc.as_ref().ok_or("No PDF loaded.")?;
        doc.translatable_segments.clone()
    };

    {
        let mut epub = state.epub_document.write().await;
        *epub = Some(EpubDocument {
            original_path: String::new(),
            filename: String::new(),
            segments: segments.clone(),
            translations: vec![None; segments.len()],
            marked_sections: vec![MarkedSection {
                spine_path: "root".to_string(),
                marked_xhtml: String::new(),
                segment_start: 0,
                segment_count: segments.len(),
            }],
            segment_meta: vec![SegmentMeta {
                segment_type: SegmentType::Paragraph,
                keep_original: false,
                font_size: None,
                is_monospace: false,
                indent: None,
                is_italic: false,
            }; segments.len()],
            code_spans: vec![],
        });
    }

    crate::application::commands::epub::translate_epub(
        state.clone(), source_lang, target_lang, app_handle
    ).await?;

    {
        let epub = state.epub_document.read().await;
        let epub = epub.as_ref().ok_or("Translation lost")?;
        let mut pdf = state.pdf_document.write().await;
        if let Some(ref mut d) = *pdf {
            d.translations = epub.translations.clone();
        }
    }

    Ok(())
}

// -- Helpers --

/// Font size by heading level (points).
fn heading_size(level: u8) -> f32 {
    match level {
        1 => 20.0,
        2 => 17.0,
        3 => 14.0,
        4 => 12.0,
        5 => 11.0,
        _ => 11.0,
    }
}

/// Vertical spacing (points) to insert before a heading.
fn heading_spacing_before(level: u8) -> f32 {
    match level {
        1 => 24.0,
        2 => 20.0,
        3 => 16.0,
        4 => 12.0,
        _ => 8.0,
    }
}

/// Vertical spacing (points) to insert after a heading.
fn heading_spacing_after(level: u8) -> f32 {
    match level {
        1 => 12.0,
        2 => 10.0,
        3 => 8.0,
        _ => 6.0,
    }
}

/// Convert a char index to a byte index in the original UTF-8 string.
fn char_to_byte_idx(chars: &[char], char_idx: usize) -> usize {
    if char_idx >= chars.len() {
        return chars.iter().map(|c| c.len_utf8()).sum();
    }
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

/// Render segments directly to PDF — no markdown intermediate.
///
/// Iterates through segments + translations + segment_meta and emits
/// each one to pdf_oxide with type-appropriate rendering:
/// - Code segments: indent 24pt, no word-wrap, original text
/// - Headings: larger font + spacing
/// - Paragraphs: CJK-aware word-wrap at body size
fn render_elements_to_pdf(doc: &PdfDocumentData) -> Result<Vec<u8>, String> {
    const FONT_PATH: &str = r"C:\Windows\Fonts\msyh.ttc";
    const FONT_NAME: &str = "CJK";
    const BASE_SIZE: f32 = 11.0;
    const LINE_HEIGHT: f32 = BASE_SIZE * 1.4;
    const PAGE_W: f32 = 595.28;
    const PAGE_H: f32 = 841.89;
    const MARGIN: f32 = 72.0;

    // Load CJK font
    let font = EmbeddedFont::from_file(FONT_PATH)
        .map_err(|e| format!("Failed to load CJK font ({}): {}", FONT_PATH, e))?;
    let mut builder = DocumentBuilder::new()
        .register_embedded_font(FONT_NAME, font);
    let mono_font_name = match EmbeddedFont::from_file(r"C:\Windows\Fonts\consola.ttf") {
        Ok(mf) => {
            builder = builder.register_embedded_font("Mono", mf);
            "Mono"
        }
        Err(_) => FONT_NAME,
    };

    // Rendering state
    let mut page = Some(builder.a4_page());
    let mut y = PAGE_H - MARGIN;
    let mut indent: f32 = 0.0;

    // — Rendering macros —
    macro_rules! ensure_space {
        ($needed:expr) => {
            if y - $needed < MARGIN + 30.0 {
                page.take().unwrap().done();
                page = Some(builder.a4_page());
                y = PAGE_H - MARGIN;
            }
        };
    }

    macro_rules! emit_line {
        ($text:expr, $size:expr) => {
            let line = $text.trim_end();
            if !line.is_empty() {
                ensure_space!(LINE_HEIGHT);
                if let Some(p) = page.take() {
                    let p = p.font(FONT_NAME, $size).at(MARGIN + indent, y).text(line);
                    page = Some(p);
                    y -= LINE_HEIGHT;
                }
            }
        };
    }

    macro_rules! emit_wrapped {
        ($text:expr, $size:expr) => {
            let text = $text.trim();
            if text.is_empty() {
                ensure_space!(LINE_HEIGHT * 0.5);
                y -= LINE_HEIGHT * 0.5;
            } else {
                let content_area = PAGE_W - 2.0 * MARGIN - indent;
                // CJK full-width chars ≈ font_size wide. Subtract 2-char safety
                // margin to prevent glyphs from clipping at the page edge.
                let cpl = (content_area / $size) as usize;
                let cpl = cpl.saturating_sub(2);
                let mut remaining = text;
                while !remaining.is_empty() {
                    if remaining.chars().count() <= cpl {
                        emit_line!(remaining, $size);
                        break;
                    }
                    let chars: Vec<char> = remaining.chars().collect();
                    // Width-weighted: CJK ≈ full-width, Latin ≈ half-width.
                    // Measure cumulative width and break before overflow.
                    let mut width = 0.0_f32;
                    let mut split = 0;
                    for (i, ch) in chars.iter().enumerate() {
                        let ch_w = if *ch > '\u{7F}' {
                            $size * 0.95  // CJK / wide chars
                        } else if ch.is_ascii_whitespace() {
                            $size * 0.3  // spaces
                        } else {
                            $size * 0.55  // Latin / digits / ASCII punct
                        };
                        width += ch_w;
                        if width > content_area && i > 0 {
                            split = i;
                            break;
                        }
                        split = i + 1;
                    }
                    if split == 0 { split = 1; } // at least one char
                    let line: String = chars[..split].iter().collect();
                    emit_line!(&line, $size);
                    let byte = char_to_byte_idx(&chars, split);
                    remaining = &remaining[byte..];
                }
            }
        };
    }

    // — Main loop —
    for elem in &doc.elements {
        match elem {
            PdfElement::TextSegment { text, meta, segment_idx } => {
                let translated = doc.translations.get(*segment_idx)
                    .and_then(|t| t.as_deref())
                    .unwrap_or(text);
                let saved_indent = indent;
                indent = meta.indent.unwrap_or(0.0).max(0.0);
                match &meta.segment_type {
                    SegmentType::Heading { level } => {
                        let size = heading_size(*level);
                        y -= heading_spacing_before(*level);
                        emit_wrapped!(translated, size);
                        y -= heading_spacing_after(*level);
                    }
                    _ => {
                        let size = if meta.is_italic { (BASE_SIZE - 2.0).max(8.0) } else { BASE_SIZE };
                        emit_wrapped!(translated, size);
                        y -= 6.0;
                    }
                }
                indent = saved_indent;
            }
            PdfElement::CodeBlock { text, font_size } => {
                let code_size = (*font_size).min(10.0_f32).max(9.0_f32);
                let saved = indent;
                indent += 30.0;
                y -= 4.0_f32;
                let line_count = text.lines().filter(|l| !l.is_empty()).count() as f32;
                let block_h = line_count * LINE_HEIGHT + 2.0_f32;
                ensure_space!(block_h);
                if let Some(p) = page.take() {
                    // Thin left border bar (content-level rect, not annotation)
                    // y = text baseline. Text top ≈ y + code_size. Align rect top with text top.
                    let p = p.rect(MARGIN + 18.0_f32, y - block_h + code_size, 3.0_f32, block_h);
                    page = Some(p);
                }
                for line in text.lines() {
                    if !line.is_empty() {
                        ensure_space!(LINE_HEIGHT);
                        if let Some(p) = page.take() {
                            let p = p.font(mono_font_name, code_size).at(MARGIN + indent, y).text(line);
                            page = Some(p);
                            y -= LINE_HEIGHT;
                        }
                    }
                }
                indent = saved;
                y -= 6.0;
            }
            PdfElement::Image { data, width, height, caption, .. } => {
                let content_w = PAGE_W - 2.0 * MARGIN;
                let (w, h) = if *width > content_w {
                    let scale = content_w / *width;
                    (*width * scale, *height * scale)
                } else { (*width, *height) };
                ensure_space!(h);
                if let Some(p) = page.take() {
                    let rect = pdf_oxide::geometry::Rect::new(MARGIN, y - h, w, h);
                    match p.image_from_bytes(data, rect) {
                        Ok(p2) => { page = Some(p2); y -= h + 6.0; }
                        Err(_) => { page = Some(builder.a4_page()); y = PAGE_H - MARGIN; }
                    }
                }
                if let Some(cap) = caption {
                    ensure_space!(BASE_SIZE * 1.2);
                    if let Some(p) = page.take() {
                        let p = p.font(FONT_NAME, 9.0_f32).at(MARGIN + 24.0, y).text(cap.as_str());
                        page = Some(p);
                        y -= BASE_SIZE * 1.2;
                    }
                }
            }
        }
    }

    // — Finish last page —
    if let Some(p) = page.take() {
        p.done();
    }

    builder
        .build()
        .map_err(|e| format!("Failed to build PDF: {}", e))
}
