import { FluentProvider, webLightTheme } from "@fluentui/react-components";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TranslateConfig {
  source_lang: string;
  target_lang: string;
}

interface TranslationResult {
  translated_text: string;
  think_content: string | null;
  prompt_tokens: number;
  completion_tokens: number;
  model: string;
  finish_reason: string | null;
}

function App() {
  const [sourceText, setSourceText] = useState("");
  const [translatedText, setTranslatedText] = useState("");
  const [sourceLang, setSourceLang] = useState("zh-Hans");
  const [targetLang, setTargetLang] = useState("en");
  const [isTranslating, setIsTranslating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTranslate = async () => {
    if (!sourceText.trim()) return;

    setIsTranslating(true);
    setError(null);

    try {
      const result = await invoke<TranslationResult>("translate_text", {
        text: sourceText,
        sourceLang,
        targetLang,
      });
      setTranslatedText(result.translated_text);
    } catch (err) {
      setError(err as string);
    } finally {
      setIsTranslating(false);
    }
  };

  return (
    <FluentProvider theme={webLightTheme}>
      <div style={{ padding: "20px", maxWidth: "800px", margin: "0 auto" }}>
        <h1
          style={{ fontSize: "24px", fontWeight: "bold", marginBottom: "20px" }}
        >
          🤖 MindRush - AI Mind Rush
        </h1>

        {/* Language Selection */}
        <div style={{ display: "flex", gap: "10px", marginBottom: "20px" }}>
          <select
            value={sourceLang}
            onChange={(e) => setSourceLang(e.target.value)}
            style={{
              padding: "8px",
              borderRadius: "4px",
              border: "1px solid #ccc",
            }}
          >
            <option value="zh-Hans">简体中文</option>
            <option value="zh-Hant">繁體中文</option>
            <option value="ja">日本語</option>
            <option value="ko">한국어</option>
            <option value="en">English</option>
          </select>

          <span style={{ lineHeight: "32px" }}>→</span>

          <select
            value={targetLang}
            onChange={(e) => setTargetLang(e.target.value)}
            style={{
              padding: "8px",
              borderRadius: "4px",
              border: "1px solid #ccc",
            }}
          >
            <option value="en">English</option>
            <option value="zh-Hans">简体中文</option>
            <option value="zh-Hant">繁體中文</option>
            <option value="ja">日本語</option>
            <option value="ko">한국어</option>
          </select>

          <button
            onClick={handleTranslate}
            disabled={isTranslating}
            style={{
              padding: "8px 16px",
              backgroundColor: isTranslating ? "#ccc" : "#0078d4",
              color: "white",
              border: "none",
              borderRadius: "4px",
              cursor: isTranslating ? "not-allowed" : "pointer",
            }}
          >
            {isTranslating ? "Translating..." : "Translate"}
          </button>
        </div>

        {/* Source Text */}
        <div style={{ marginBottom: "20px" }}>
          <label
            style={{ display: "block", marginBottom: "8px", fontWeight: "500" }}
          >
            Source Text
          </label>
          <textarea
            value={sourceText}
            onChange={(e) => setSourceText(e.target.value)}
            placeholder="Enter text to translate..."
            rows={6}
            style={{
              width: "100%",
              padding: "12px",
              borderRadius: "4px",
              border: "1px solid #ccc",
              resize: "vertical",
              fontFamily: "inherit",
            }}
          />
        </div>

        {/* Translated Text */}
        <div>
          <label
            style={{ display: "block", marginBottom: "8px", fontWeight: "500" }}
          >
            Translation Result
          </label>
          <textarea
            value={translatedText}
            readOnly
            placeholder="Translation will appear here..."
            rows={6}
            style={{
              width: "100%",
              padding: "12px",
              borderRadius: "4px",
              border: "1px solid #ccc",
              resize: "vertical",
              fontFamily: "inherit",
              backgroundColor: "#f9f9f9",
            }}
          />
        </div>

        {/* Error Display */}
        {error && (
          <div
            style={{
              marginTop: "20px",
              padding: "12px",
              backgroundColor: "#fee",
              border: "1px solid #f00",
              borderRadius: "4px",
              color: "#c00",
            }}
          >
            {error}
          </div>
        )}

        {/* Footer */}
        <div
          style={{
            marginTop: "40px",
            padding: "20px",
            textAlign: "center",
            color: "#666",
          }}
        >
          <p>MindRush v0.1.0 - Rust + Tauri + React</p>
          <p style={{ fontSize: "12px" }}>Translation powered by AI</p>
        </div>
      </div>
    </FluentProvider>
  );
}

export default App;
