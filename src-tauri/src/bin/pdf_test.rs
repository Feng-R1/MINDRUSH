// Headless PDF reader test binary.
//
// Tests span-based extraction with pdf_oxide 0.3.55.
// Opens a PDF, extracts spans with adaptive merging,
// groups into paragraphs, and prints metadata.
//
// Does NOT call any LLM API (no translate step).

use pdf_oxide::extractors::SpanMergingConfig;
use pdf_oxide::layout::TextSpan;
use pdf_oxide::PdfDocument;

const TEST_PDF: &str =
    "D:\\Projects\\rust_workspace\\examples\\AiNieer\\test_books\\C++23 Best Practices-2024-英文版.pdf";

fn group_spans_to_paragraphs(spans: &[TextSpan]) -> Vec<String> {
    if spans.is_empty() {
        return vec![];
    }
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_text = String::new();
    let mut current_y: Option<f32> = None;
    for span in spans {
        if span.text.trim().is_empty() {
            continue;
        }
        match current_y {
            None => {
                current_text = span.text.clone();
                current_y = Some(span.bbox.y);
            }
            Some(prev_y) => {
                if (span.bbox.y - prev_y).abs() <= 4.0_f32 {
                    current_text.push_str(&span.text);
                } else {
                    let trimmed = std::mem::take(&mut current_text);
                    let trimmed = trimmed.trim().to_string();
                    if !trimmed.is_empty() {
                        paragraphs.push(trimmed);
                    }
                    current_text = span.text.clone();
                }
            }
        }
        current_y = Some(span.bbox.y);
    }
    let trimmed = current_text.trim().to_string();
    if !trimmed.is_empty() {
        paragraphs.push(trimmed);
    }
    paragraphs
}

fn main() {
    println!("=== PDF Test Binary (span-based) ===\n");

    // ---- Step 1: Open PDF ----
    println!("--- Step 1: Opening PDF with pdf_oxide 0.3.55 ---");
    let path = TEST_PDF;

    let doc = match PdfDocument::open(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: Failed to open PDF: {}", e);
            std::process::exit(1);
        }
    };

    let page_count = match doc.page_count() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("ERROR: Failed to get page count: {}", e);
            std::process::exit(1);
        }
    };
    println!("  PDF opened successfully ({} pages)", page_count);

    // ---- Step 2: Extract spans with adaptive merging ----
    println!("\n--- Step 2: Extracting spans ---");
    let mut all_spans: Vec<TextSpan> = Vec::new();
    for i in 0..page_count {
        let config = SpanMergingConfig::adaptive();
        match doc.extract_spans_with_config(i, config) {
            Ok(spans) => all_spans.extend(spans),
            Err(e) => {
                eprintln!("ERROR: Failed to extract spans from page {}: {}", i, e);
                std::process::exit(1);
            }
        }
    }
    println!("  Total spans extracted: {}", all_spans.len());

    // ---- Step 3: Group into paragraphs ----
    println!("\n--- Step 3: Grouping spans into paragraphs ---");
    let paragraphs = group_spans_to_paragraphs(&all_spans);
    println!("  Total paragraphs: {}", paragraphs.len());

    // ---- Step 4: Print first 5 paragraphs as preview ----
    println!("\n--- First 5 paragraphs preview ---\n");
    for (i, para) in paragraphs.iter().take(5).enumerate() {
        let preview: String = para.chars().take(200).collect();
        println!("[{}] {}", i + 1, preview);
        if para.len() > 200 {
            println!("    ... (truncated, {} chars total)", para.len());
        }
    }

    println!("\n=== PDF test completed successfully ===");
}
