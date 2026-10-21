export type Provider = 'openai' | 'anthropic' | 'local';

export interface LlmConfig {
  provider_type: 'OpenAI' | 'Anthropic' | 'Google' | 'AmazonBedrock' | 'LocalLLM' | 'Sakura';
  model: string;
  api_key: string;
  base_url: string | null;
  max_tokens: number | null;
  temperature: number;
  timeout_secs: number;
}

export interface ConfigEntry { name: string; config: LlmConfig }
export interface ConfigFile {
  system_prompt: string;
  glossary: string;
  llm_configs: ConfigEntry[];
  active_llm_index: number;
}
export interface TranslationResult {
  translated_text: string;
  model: string;
  prompt_tokens: number;
  completion_tokens: number;
}
export interface HistoryItem {
  original: string;
  translated: string;
  source_lang: string;
  target_lang: string;
  tokens: number;
  timestamp: string;
}
export const languages = [
  ['zh-Hans', '简体中文'], ['zh-Hant', '繁體中文'], ['en', '英语'],
  ['ja', '日语'], ['ko', '韩语'], ['fr', '法语'], ['de', '德语'],
  ['es', '西班牙语'], ['ru', '俄语'], ['pt', '葡萄牙语'], ['it', '意大利语'],
] as const;

export const errorText = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : '操作失败，请重试。';
export const isConfigured = (config: LlmConfig | null) =>
  !!config?.model.trim() && (config.provider_type === 'LocalLLM' ? !!config.base_url?.trim() : !!config.api_key.trim());
