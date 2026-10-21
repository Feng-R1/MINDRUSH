import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ConfigEntry, LlmConfig, Provider, errorText } from './types';

interface Props {
  current: LlmConfig;
  entries: ConfigEntry[];
  activeIndex: number;
  systemPrompt: string;
  glossary: string;
  onSelect: (index: number) => Promise<void>;
  onSaved: () => Promise<void>;
  onClose: () => void;
  setBusy: (value: boolean) => void;
}

export function SettingsPanel(props: Props) {
  const [provider, setProvider] = useState<Provider>(props.current.provider_type === 'Anthropic' ? 'anthropic' : props.current.provider_type === 'LocalLLM' ? 'local' : 'openai');
  const [model, setModel] = useState(props.current.model);
  const [apiKey, setApiKey] = useState('');
  const [savedKey, setSavedKey] = useState(props.current.api_key);
  const [baseUrl, setBaseUrl] = useState(props.current.base_url ?? '');
  const [systemPrompt, setSystemPrompt] = useState(props.systemPrompt);
  const [glossary, setGlossary] = useState(props.glossary);
  const [newName, setNewName] = useState('');
  const [busy, setLocalBusy] = useState(false);
  const setBusy = (value: boolean) => { setLocalBusy(value); props.setBusy(value); };
  const [error, setError] = useState('');

  const save = async () => {
    if (!model.trim()) { setError('请填写模型名称。'); return; }
    if (provider !== 'local' && !apiKey.trim() && !savedKey) { setError('请填写 API Key。'); return; }
    if (provider === 'local' && !baseUrl.trim()) { setError('请填写本地模型的接口地址，例如 http://localhost:11434/v1。'); return; }
    if (baseUrl.trim()) {
      try {
        const url = new URL(baseUrl.trim());
        if (!['http:', 'https:'].includes(url.protocol)) throw new Error();
      } catch { setError('接口地址需要以 http:// 或 https:// 开头。'); return; }
    }
    setBusy(true);
    setError('');
    try {
      const args = {
        provider, model: model.trim(), apiKey: apiKey.trim() || savedKey,
        baseUrl: baseUrl.trim() || null, temperature: props.current.temperature, maxTokens: props.current.max_tokens,
      };
      if (newName.trim()) await invoke('add_llm_config', { ...args, name: newName.trim() });
      else await invoke('update_llm_config', args);
      await invoke('update_system_prompt', { prompt: systemPrompt });
      await invoke('save_glossary', { glossary });
      await invoke('save_config');
      await props.onSaved();
    } catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  };

  const select = async (index: number) => {
    setBusy(true);
    try { await props.onSelect(index); }
    catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  };

  return (
    <section className="settings-panel" aria-labelledby="settings-title">
      <div className="settings-heading"><div><h2 id="settings-title">模型与翻译设置</h2></div><button className="icon-button" onClick={props.onClose} disabled={busy} aria-label="关闭模型设置">×</button></div>
      <p className="settings-help">使用自己的模型服务。API Key 以明文保存在本机配置中，只随请求发送至你设置的接口；费用由服务商收取。</p>
      <fieldset disabled={busy} className="settings-grid">
        <label className="wide-field">已保存的配置<select value={props.activeIndex} onChange={(event) => void select(Number(event.target.value))}>{props.entries.map((entry, index) => <option value={index} key={index}>{entry.name === 'Default' ? '默认配置' : entry.name}</option>)}</select></label>
        <label>服务类型<select value={provider} onChange={(event) => { setProvider(event.target.value as Provider); setSavedKey(''); setApiKey(''); setBaseUrl(''); }}><option value="openai">OpenAI 兼容接口</option><option value="anthropic">Anthropic</option><option value="local">本地模型（OpenAI 兼容）</option></select></label>
        <label>模型名称<input value={model} onChange={(event) => setModel(event.target.value)} placeholder="填写服务商提供的模型名称" /></label>
        <label className="wide-field">API Key <span className="field-note">{savedKey ? '留空保留已有密钥' : provider === 'local' ? '本地服务可留空' : '必填'}</span><input type="password" autoComplete="new-password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={savedKey ? '已有密钥，留空不修改' : '粘贴 API Key'} /></label>
        <label className="wide-field">接口地址 <span className="field-note">{provider === 'local' ? '必填' : '留空使用官方地址'}</span><input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder={provider === 'anthropic' ? 'https://api.anthropic.com' : provider === 'local' ? 'http://localhost:11434/v1' : 'https://api.openai.com/v1'} /></label>
        <details className="wide-field advanced-settings">
          <summary>术语表与高级设置</summary>
          <label>翻译提示词<textarea value={systemPrompt} onChange={(event) => setSystemPrompt(event.target.value)} rows={3} /></label>
          <label>术语表<textarea value={glossary} onChange={(event) => setGlossary(event.target.value)} rows={3} placeholder={'每行一个术语，例如：\nAI agent = 人工智能智能体'} /></label>
          <label>另存为新配置（可选）<input value={newName} onChange={(event) => setNewName(event.target.value)} placeholder="例如：本地翻译模型；留空修改当前配置" /></label>
        </details>
      </fieldset>
      {error && <p className="feedback error-feedback" role="alert">{error}</p>}
      <div className="settings-footer"><span>服务类型切换后，请重新填写对应地址和密钥。</span><button className="primary-button compact" onClick={() => void save()} disabled={busy}>{busy ? '正在保存…' : '保存设置'}</button></div>
    </section>
  );
}
