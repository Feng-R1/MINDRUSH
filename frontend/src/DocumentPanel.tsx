import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { errorText } from './types';

interface LoadInfo { filename: string; segment_count: number }
interface Progress { current_segment: number; total_segments: number }
interface Props {
  sourceLang: string;
  targetLang: string;
  configured: boolean;
  settingsVersion: number;
  busy: boolean;
  setBusy: (value: boolean) => void;
  onConfigure: () => void;
}

export function DocumentPanel(props: Props) {
  const [path, setPath] = useState('');
  const [info, setInfo] = useState<LoadInfo | null>(null);
  const [completed, setCompleted] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [loadedSettings, setLoadedSettings] = useState('');
  const [output, setOutput] = useState('');
  const [error, setError] = useState('');
  const signature = [props.sourceLang, props.targetLang, props.settingsVersion].join(':');
  const kind = path.toLowerCase().endsWith('.pdf') ? 'pdf' : path.toLowerCase().endsWith('.md') ? 'md' : 'epub';

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisten = listen<Progress>('epub-translation-progress', (event) => { if (!disposed) setProgress(event.payload); });
    return () => { disposed = true; void unlisten.then((stop) => stop()); };
  }, []);

  useEffect(() => { setCompleted(false); setProgress(null); setOutput(''); }, [signature]);

  const load = async (selectedPath: string) => {
    const extension = selectedPath.split('.').pop()?.toLowerCase();
    if (!extension || !['md', 'epub', 'pdf'].includes(extension)) { setError('请选择 EPUB、PDF 或 Markdown 文件。'); return; }
    props.setBusy(true);
    setError(''); setCompleted(false); setOutput(''); setInfo(null); setProgress(null);
    try {
      const loaded = await invoke<LoadInfo>('read_' + extension, { path: selectedPath });
      setPath(selectedPath); setInfo(loaded); setLoadedSettings(signature);
      if (!loaded.segment_count) setError('未找到可翻译文本。扫描版 PDF 需要先通过 OCR 转为可选中文本。');
    } catch (error) { setError(errorText(error)); }
    finally { props.setBusy(false); }
  };

  const choose = async () => {
    try {
      const selected = await open({ multiple: false, filters: [{ name: '可翻译文档', extensions: ['epub', 'pdf', 'md'] }] });
      if (typeof selected === 'string') await load(selected);
    } catch (error) { setError(errorText(error)); }
  };

  const translate = async () => {
    if (!props.configured) { props.onConfigure(); return; }
    if (!info) return;
    props.setBusy(true); setError(''); setOutput(''); setCompleted(false);
    try {
      if (loadedSettings !== signature) {
        await invoke('read_' + kind, { path });
        setLoadedSettings(signature);
      }
      await invoke(kind === 'pdf' ? 'translate_pdf' : 'translate_epub', { sourceLang: props.sourceLang, targetLang: props.targetLang });
      setCompleted(true);
      setProgress({ current_segment: info.segment_count, total_segments: info.segment_count });
    } catch (error) { setError(errorText(error)); }
    finally { props.setBusy(false); }
  };

  const exportDocument = async () => {
    props.setBusy(true); setError('');
    try { setOutput(await invoke<string>('export_' + kind)); }
    catch (error) { setError(errorText(error)); }
    finally { props.setBusy(false); }
  };

  return (
    <div className="document-panel">
      <div className="document-picker"><span className="document-symbol" aria-hidden="true">▤</span><h2>把整份文档交给 MindRush</h2><p>支持 EPUB、PDF、Markdown。导出译文到原文件旁，保留原文件。</p><button className="primary-button" onClick={() => void choose()} disabled={props.busy}>选择文档</button></div>
      <details className="path-entry"><summary>通过完整文件路径打开</summary><div><input aria-label="文档完整路径" value={path} onChange={(event) => { setPath(event.target.value); setInfo(null); setCompleted(false); }} disabled={props.busy} placeholder="输入完整的本机文件路径" /><button className="settings-button" onClick={() => void load(path)} disabled={props.busy || !path.trim()}>读取文件</button></div></details>
      {info && <div className="document-info"><div><strong>{info.filename}</strong><p>{info.segment_count} 个可翻译片段</p></div><div className="document-actions"><button className="primary-button" disabled={props.busy || !info.segment_count} onClick={() => void translate()}>{props.busy ? '正在处理…' : error ? '重试文档翻译' : completed ? '重新翻译' : '翻译文档'}</button><button className="settings-button" disabled={props.busy || !completed} onClick={() => void exportDocument()}>导出译文</button></div></div>}
      {progress && <div className="document-progress" role="status"><progress max={Math.max(progress.total_segments, 1)} value={progress.current_segment} /><span>{completed ? '翻译完成' : '已翻译'} {progress.current_segment} / {progress.total_segments} 个片段</span></div>}
      {output && <p className="feedback success-feedback output-path" role="status">译文已导出：{output}</p>}
      {error && <p className="feedback error-feedback" role="alert">{error}</p>}
      <p className="document-note">EPUB 会尝试保留章节和资源；PDF 使用文本重排，复杂版式可能变化，暂不支持扫描件 OCR。长文档会发起多次模型调用，费用取决于所选模型。</p>
    </div>
  );
}
