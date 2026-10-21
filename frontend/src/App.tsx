import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { DocumentPanel } from './DocumentPanel';
import { SettingsPanel } from './SettingsPanel';
import { ConfigEntry, ConfigFile, HistoryItem, LlmConfig, TranslationResult, errorText, isConfigured, languages } from './types';
import metadata from '../package.json';

const defaultConfig: LlmConfig = {
  provider_type: 'OpenAI', model: 'gpt-4o-mini', api_key: '', base_url: null,
  temperature: 0.3, max_tokens: 4096, timeout_secs: 120,
};

function App() {
  const [sourceText, setSourceText] = useState('');
  const [translatedText, setTranslatedText] = useState('');
  const [sourceLang, setSourceLang] = useState('en');
  const [targetLang, setTargetLang] = useState('zh-Hans');
  const [busy, setBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [showSettings, setShowSettings] = useState(false);
  const [config, setConfig] = useState<LlmConfig>(defaultConfig);
  const [entries, setEntries] = useState<ConfigEntry[]>([{ name: '默认配置', config: defaultConfig }]);
  const [activeIndex, setActiveIndex] = useState(0);
  const [settingsVersion, setSettingsVersion] = useState(0);
  const [systemPrompt, setSystemPrompt] = useState('');
  const [glossary, setGlossary] = useState('');
  const [tab, setTab] = useState<'text' | 'document' | 'history'>('text');
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const configured = isConfigured(config);

  const refreshConfig = async () => {
    const saved = await invoke<ConfigFile | null>('load_config');
    const current = await invoke<LlmConfig>('get_llm_config');
    const allEntries = await invoke<ConfigEntry[]>('list_llm_configs');
    setConfig(current); setEntries(allEntries);
    setActiveIndex(saved?.active_llm_index ?? 0);
    setSystemPrompt(saved?.system_prompt ?? 'You are a professional translator. Translate accurately. Return only the translation, preserving paragraphs and formatting.');
    setGlossary(saved?.glossary ?? '');
    return current;
  };

  useEffect(() => {
    if (!isTauri()) { setReady(true); setError('请在 MindRush 桌面应用中使用翻译功能。网页预览仅展示界面。'); return; }
    void refreshConfig()
      .then((current) => setShowSettings(!isConfigured(current)))
      .catch((error) => { setError('读取本机配置失败：' + errorText(error)); setShowSettings(true); })
      .finally(() => setReady(true));
  }, []);

  const configure = () => { setShowSettings(true); setError('请先保存模型配置，再开始翻译。'); };

  const handleTranslate = async () => {
    if (!sourceText.trim() || busy || !ready) return;
    if (!configured) { configure(); return; }
    setBusy(true); setError(''); setNotice('');
    try {
      const result = await invoke<TranslationResult>('translate_text', { text: sourceText, sourceLang, targetLang });
      setTranslatedText(result.translated_text);
      setNotice('翻译完成 · ' + result.model);
      await invoke('add_history', { original: sourceText, translated: result.translated_text, sourceLang, targetLang, tokens: result.prompt_tokens + result.completion_tokens });
    } catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  };

  const switchLanguages = () => {
    setSourceLang(targetLang); setTargetLang(sourceLang);
    if (translatedText) { setSourceText(translatedText); setTranslatedText(sourceText); }
    setError(''); setNotice('');
  };

  const savedSettings = async () => {
    await refreshConfig();
    setSettingsVersion((value) => value + 1); setShowSettings(false); setError('');
    setNotice('模型与翻译设置已保存。');
  };

  const selectConfig = async (index: number) => {
    await invoke('set_active_llm', { index });
    await invoke('save_config');
    await refreshConfig();
    setSettingsVersion((value) => value + 1);
  };

  const copyTranslation = async () => {
    try { await navigator.clipboard.writeText(translatedText); setNotice('译文已复制。'); }
    catch { setError('复制失败，请手动选择译文复制。'); }
  };

  const openHistory = async () => {
    setTab('history'); setError(''); setNotice('');
    try { setHistory((await invoke<HistoryItem[]>('get_history')).reverse()); }
    catch (error) { setError(errorText(error)); }
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <a className="brand" href="#home" aria-label="MindRush 首页"><span className="brand-mark" aria-hidden="true">M</span><span>MindRush</span></a>
        <div className="topbar-actions">
          <span className={'service-state' + (configured ? ' is-ready' : '')}><span className="state-dot" />{!ready ? '读取配置…' : configured ? '模型已配置' : '需要配置模型'}</span>
          <button className="settings-button" disabled={busy || !ready} onClick={() => { setShowSettings((value) => !value); setError(''); }} aria-expanded={showSettings}><span aria-hidden="true">⚙</span> 模型设置</button>
        </div>
      </header>
      <section className="workspace" id="home">
        <div className="intro">
          <p className="eyebrow">AI 文本与文档翻译</p>
          <h1>让每一段外文，读起来都自然。</h1>
          <p className="intro-copy">从一段文字到一本书，用你自己的模型服务，完成翻译与导出。</p>
        </div>
        {showSettings && <SettingsPanel key={activeIndex + ':' + settingsVersion} current={config} entries={entries} activeIndex={activeIndex} systemPrompt={systemPrompt} glossary={glossary} onSelect={selectConfig} onSaved={savedSettings} onClose={() => setShowSettings(false)} setBusy={setBusy} />}

        <section className="translation-workspace" aria-label="翻译工作台">
          <nav className="workspace-tabs" aria-label="功能切换">
            <button className={tab === 'text' ? 'active' : ''} disabled={busy} aria-current={tab === 'text' ? 'page' : undefined} onClick={() => { setTab('text'); setNotice(''); setError(''); }}>文本翻译</button>
            <button className={tab === 'document' ? 'active' : ''} disabled={busy} aria-current={tab === 'document' ? 'page' : undefined} onClick={() => { setTab('document'); setNotice(''); setError(''); }}>文档翻译</button>
            <button className={tab === 'history' ? 'active' : ''} disabled={busy} aria-current={tab === 'history' ? 'page' : undefined} onClick={() => void openHistory()}>本次记录</button>
          </nav>
          {tab !== 'history' && <div className="language-bar">
            <label className="language-select"><span className="sr-only">源语言</span><select value={sourceLang} disabled={busy} onChange={(event) => setSourceLang(event.target.value)}>{languages.map(([code, label]) => <option key={code} value={code}>{label}</option>)}</select></label>
            <button className="swap-button" disabled={busy} onClick={switchLanguages} aria-label="交换源语言和目标语言">⇄</button>
            <label className="language-select"><span className="sr-only">目标语言</span><select value={targetLang} disabled={busy} onChange={(event) => setTargetLang(event.target.value)}>{languages.map(([code, label]) => <option key={code} value={code}>{label}</option>)}</select></label>
            <span className="language-hint">选择原文语言与目标语言</span>
          </div>}
          <div hidden={tab !== 'text'}>
            <div className="editor-grid">
              <section className="editor-pane source-pane" aria-label="原文输入">
                <div className="pane-heading"><span>原文</span><span className="pane-meta">{sourceText.length.toLocaleString()} 字符</span></div>
                <textarea value={sourceText} disabled={busy} onChange={(event) => { setSourceText(event.target.value); setError(''); }} onKeyDown={(event) => { if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); void handleTranslate(); } }} placeholder="把想翻译的内容放在这里…" aria-label="输入原文" />
                <div className="pane-footer"><span>Ctrl + Enter 开始翻译</span><button className="text-button" onClick={() => { setSourceText(''); setTranslatedText(''); setError(''); setNotice(''); }} disabled={busy || (!sourceText && !translatedText)}>清空内容</button></div>
              </section>
              <section className="editor-pane result-pane" aria-label="译文区域">
                <div className="pane-heading"><span>译文</span>{translatedText && <button className="text-button copy-button" onClick={() => void copyTranslation()}>复制译文</button>}</div>
                <textarea value={translatedText} disabled={busy} onChange={(event) => setTranslatedText(event.target.value)} placeholder="译文会显示在这里…" aria-label="翻译结果" />
                <div className="pane-footer"><span>结果可直接编辑</span><span className="result-spark" aria-hidden="true">✳</span></div>
              </section>
            </div>
            <div className="action-row"><p className="privacy-note">文本仅发送至你配置的模型接口</p><button className="primary-button" onClick={() => void handleTranslate()} disabled={busy || showSettings || !ready || !sourceText.trim()}>{busy ? <><span className="spinner" />正在翻译…</> : '开始翻译'}</button></div>
          </div>
          <div hidden={tab !== 'document'}><DocumentPanel sourceLang={sourceLang} targetLang={targetLang} configured={configured} settingsVersion={settingsVersion} busy={busy || showSettings || !ready} setBusy={setBusy} onConfigure={configure} /></div>
      {tab === 'history' && <div className="history-panel">
            <div className="history-heading"><p>记录保存在本机运行期间，退出应用后不会保留。</p><button className="text-button" disabled={!history.length} onClick={() => { void invoke('clear_history').then(() => setHistory([])).catch((error) => setError(errorText(error))); }}>清空记录</button></div>
            {!history.length && <p className="empty-state">还没有翻译记录。先翻译一段文字吧。</p>}
            {history.map((item, index) => <article className="history-item" key={index}><time>{item.timestamp}</time><p>{item.original}</p><blockquote>{item.translated}</blockquote></article>)}
          </div>}
          {error && <p className="feedback error-feedback" role="alert">{error}</p>}
          {notice && !error && <p className="feedback success-feedback" role="status">{notice}</p>}
        </section>
        <footer className="app-footer"><span>MindRush <span className="footer-version">v{metadata.version}</span></span><span>翻译由你选择的 AI 模型完成</span></footer>
      </section>
    </main>
  );
}

export default App;
