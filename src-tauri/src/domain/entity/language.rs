// Domain entity: Language enumeration

use serde::{Deserialize, Serialize};

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Language {
    ChineseSimplified = 0,
    ChineseTraditional = 1,
    Japanese = 2,
    Korean = 3,
    English = 4,
    German = 5,
    French = 6,
    Spanish = 7,
    Russian = 8,
    Portuguese = 9,
    Italian = 10,
    Dutch = 11,
    Polish = 12,
    Czech = 13,
    Hungarian = 14,
    Unknown = 99,
}

impl Language {
    /// Detect language from text using heuristics
    pub fn detect_from_text(text: &str) -> (Self, f32) {
        let has_cjk = text.chars().any(|c| {
            let code = c as u32;
            (code >= 0x4E00 && code <= 0x9FFF)       // CJK Unified Ideographs
         || (code >= 0x3400 && code <= 0x4DBF)       // CJK Unified Ideographs Extension A
        });
        let has_japanese = text.chars().any(|c| {
            let code = c as u32;
            (code >= 0x3040 && code <= 0x309F)       // Hiragana
         || (code >= 0x30A0 && code <= 0x30FF)       // Katakana
        });
        let has_korean = text.chars().any(|c| {
            let code = c as u32;
            code >= 0xAC00 && code <= 0xD7AF       // Korean syllables
        });

        if has_japanese { return (Language::Japanese, 0.9); }
        if has_korean { return (Language::Korean, 0.9); }
        if has_cjk { return (Language::ChineseSimplified, 0.8); }

        // Latin detection
        let has_latin = text.chars().any(|c| {
            let code = c as u32;
            (code >= 0x0041 && code <= 0x007A) || (code >= 0x00C0 && code <= 0x024F)
        });
        if has_latin { return (Language::English, 0.5); }

        (Language::Unknown, 0.0)
    }

    /// Get IETF language tag
    pub fn to_ietf_tag(&self) -> &'static str {
        match self {
            Language::ChineseSimplified => "zh-Hans",
            Language::ChineseTraditional => "zh-Hant",
            Language::Japanese => "ja",
            Language::Korean => "ko",
            Language::English => "en",
            Language::German => "de",
            Language::French => "fr",
            Language::Spanish => "es",
            Language::Russian => "ru",
            Language::Portuguese => "pt",
            Language::Italian => "it",
            Language::Dutch => "nl",
            Language::Polish => "pl",
            Language::Czech => "cs",
            Language::Hungarian => "hu",
            Language::Unknown => "und",
        }
    }

    /// Parse from IETF language tag
    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "zh" | "zh-hans" | "chs" | "zh-cn" => Some(Language::ChineseSimplified),
            "zh-tw" | "zh-hant" | "cht" => Some(Language::ChineseTraditional),
            "ja" | "jpn" => Some(Language::Japanese),
            "ko" | "kor" => Some(Language::Korean),
            "en" | "eng" => Some(Language::English),
            "de" | "deu" => Some(Language::German),
            "fr" | "fra" => Some(Language::French),
            "es" | "spa" => Some(Language::Spanish),
            "ru" | "rus" => Some(Language::Russian),
            "pt" | "por" => Some(Language::Portuguese),
            "it" | "ita" => Some(Language::Italian),
            "nl" | "dut" => Some(Language::Dutch),
            "pl" | "pol" => Some(Language::Polish),
            "cs" | "cze" => Some(Language::Czech),
            "hu" | "hun" => Some(Language::Hungarian),
            _ => None,
        }
    }
}