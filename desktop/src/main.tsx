import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './style.css';

type Snapshot = { day: string; startedAt: number | null; endedAt: number | null; status: string; serverNow: number; captureCount: number };
type Capture = { id: number; title: string; createdAt: number };
type Settings = { autostart: boolean; shortcut: string };
const kind = new URLSearchParams(location.search).get('window') ?? 'main';
const labels: Record<string,string> = { idle: '尚未上班', working: '上班中', overtime: '加班中', finished: '已下班' };
function duration(seconds: number) { return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(n => String(n).padStart(2, '0')).join(':'); }
function App() {
  const [data, setData] = useState<Snapshot>();
  const [items, setItems] = useState<Capture[]>([]);
  const [settings, setSettings] = useState<Settings>();
  const [key, setKey] = useState('Ctrl+Shift+Space');
  const [text, setText] = useState('');
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState(Date.now() / 1000);
  async function refresh() {
    setData(await invoke<Snapshot>('snapshot'));
    if (kind === 'main') { setItems(await invoke<Capture[]>('captures')); const s = await invoke<Settings>('settings'); setSettings(s); setKey(s.shortcut || 'Ctrl+Shift+Space'); }
  }
  async function loadDraft() { const d = await invoke<{text:string;notice:string}>('draft'); setText(d.text); setNotice(d.notice); setError(''); }
  useEffect(() => {
    if (!isTauri()) { setError('请通过桌面应用打开。浏览器预览不连接本地数据。'); return; }
    let disposed = false; const cleanups: (()=>void)[] = [];
    const attach = async () => {
      for (const [event, handler] of [['domain-changed', refresh], ...(kind === 'quick_add' ? [['draft-changed', loadDraft]] : [])] as [string,()=>Promise<void>][]) {
        const off = await listen(event, () => { void handler().catch(e => setError(String(e))); });
        if (disposed) off(); else cleanups.push(off);
      }
      await refresh(); if (kind === 'quick_add') await loadDraft();
    };
    void attach().catch(e => setError(String(e)));
    const timer = setInterval(() => setNow(Date.now()/1000), 1000);
    const onFocus = () => { void refresh().catch(e => setError(String(e))); };
    window.addEventListener('focus', onFocus);
    return () => { disposed = true; clearInterval(timer); cleanups.forEach(f => f()); window.removeEventListener('focus', onFocus); };
  }, []);
  async function run(action: () => Promise<unknown>) { if (busy) return; setBusy(true); setError(''); try { await action(); } catch(e) { setError(String(e)); } finally { setBusy(false); } }
  const windowAction = (action: string) => run(() => invoke('window_action', {action}));
  const elapsed = data?.startedAt == null ? 0 : Math.max(0, Math.floor((data.endedAt ?? now) - data.startedAt));
  const status = data?.status === 'working' && elapsed >= 32400 ? 'overtime' : data?.status ?? 'idle';
  const clockButton = <button disabled={busy || !data || status === 'finished'} className="primary" onClick={() => run(() => invoke('clock', {action: status === 'idle' ? 'start' : 'finish'}))}>{status === 'finished' ? '今天已下班' : status === 'idle' ? '上班打卡' : '下班打卡'}</button>;
  const progress = <div className="progress"><span style={{width: `${Math.min(100, elapsed/324)}%`}} /></div>;
  const feedback = <>{error && <p role="alert" className="error">{error}</p>}{notice && <p role="status" className="notice">{notice}</p>}</>;
  if (kind === 'widget') return <main className="widget"><header onMouseDown={e => {if (e.button === 0) void getCurrentWindow().startDragging().catch(e => setError(String(e)));}}><b>✦ 早点下班</b><span>{labels[status]}</span></header><div className="widget-time"><small>今天的时间</small><strong>{duration(elapsed)} <em>/ 9h</em></strong></div>{progress}<footer>{clockButton}<button onClick={() => windowAction('main')}>打开主面板</button><button aria-label="隐藏悬浮窗" onClick={() => windowAction('hide')}>×</button></footer>{feedback}</main>;
  if (kind === 'quick_add') return <main className="quick"><p className="eyebrow">留住灵感，早点下班</p><h1>快速收集</h1><form onSubmit={e => {e.preventDefault();void run(async () => {await invoke('add_capture',{title:text});setText('');setNotice('已加密保存到本地收集箱');await invoke('window_action',{action:'hide'});});}}><label htmlFor="capture">工作事项</label><textarea id="capture" autoFocus maxLength={2000} value={text} placeholder="输入或粘贴一条待办…" onChange={e => setText(e.target.value)} /><div className="actions"><small>{text.length} / 2000</small><button type="button" onClick={() => windowAction('hide')}>取消</button><button className="primary" disabled={busy || !text.trim()}>保存到收集箱</button></div></form>{feedback}</main>;
  return <main className="shell"><aside><div className="brand">✦</div><b>早点<br/>下班</b><span>阶段 1<br/>技术小样</span></aside><section className="content"><header className="top"><div><p className="eyebrow">让时间回到自己手里</p><h1>今天，也早点下班。</h1></div><button onClick={() => windowAction('widget')}>显示悬浮窗 ↗</button></header><section className="hero"><div className="orb"/><p>{data?.day ?? '本地工作日'} · {labels[status]}</p><h2>{duration(elapsed)}</h2><p>{status === 'finished' ? '今天辛苦了，剩下的时间属于你。' : status === 'overtime' ? '已满九小时，记得给自己留点时间。' : '专注工作，也记得准时收工。'}</p>{progress}<div className="actions">{clockButton}<small>目标 9 小时 · 关闭主面板后继续计时</small></div></section>{feedback}<div className="columns"><section className="card"><div className="section-head"><h3>本地收集箱 <small>{data?.captureCount ?? 0}</small></h3><button onClick={() => windowAction('quick')}>＋ 快速添加</button></div><p className="muted">内容经 AES-256-GCM 加密，由当前 Windows 账户解锁。</p><div className="items">{items.length ? items.map(i => <article key={i.id}><span className="dot"/><div>{i.title}<small>{new Date(i.createdAt*1000).toLocaleString('zh-CN')}</small></div></article>) : <div className="empty">此刻，收集箱空空的。<br/><small>选中文字后按快捷键，或手动添加一条。</small></div>}</div>{items.length >= 50 && <small>仅展示最近 50 条</small>}</section><section className="card"><h3>桌面设置</h3><label className="switch"><span>登录 Windows 后启动<small>静默进入托盘并显示悬浮窗</small></span><input type="checkbox" checked={settings?.autostart ?? false} disabled={busy || !settings} onChange={e => run(async () => {await invoke('set_autostart',{enabled:e.target.checked});await refresh();})}/></label><label htmlFor="shortcut">划词收集快捷键</label><div className="key-input"><input id="shortcut" value={key} onChange={e => setKey(e.target.value)}/><button disabled={busy} onClick={() => run(async () => {await invoke('set_shortcut',{key});await refresh();setNotice('快捷键已生效');})}>应用</button></div>{settings && !settings.shortcut && <p className="error">快捷键注册失败或已被占用，请更换组合键。</p>}<p className="muted">仅主动触发时读取剪贴板。原剪贴板含复杂格式时，使用手动粘贴。</p><div className="divider"/><p className="muted">点击窗口关闭按钮会隐藏到托盘。需要停止应用时，使用托盘菜单或下方按钮。</p><button className="exit" onClick={() => windowAction('exit')}>退出应用</button></section></div><footer className="foot">桌面技术小样 0.1.0-alpha.1 · 数据仅保存在此电脑 · 尚未包含完整任务与报告功能</footer></section></main>;
}
createRoot(document.getElementById('root')!).render(<App/>);
