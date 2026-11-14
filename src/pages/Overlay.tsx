import { useState, useRef } from 'react';
import {
  Monitor,
  Copy,
  Check,
  ExternalLink,
  RefreshCw,
  Palette,
  Download,
  Minimize,
  Grid,
  Layers,
  Sparkles,
} from 'lucide-react';
import { useDelaySec, useDelayMode, useIsBuffering, useNotify } from '../store/useStore';
import { useOverlay, useSettings } from '../hooks/useTauri';
import type { Settings } from '../types';
import clsx from 'clsx';

const PRESETS = [
  { id: 'minimal', name: 'Minimal', icon: Minimize },
  { id: 'modern', name: 'Modern', icon: Sparkles },
  { id: 'gaming', name: 'Gaming', icon: Grid },
  { id: 'broadcast', name: 'Broadcast', icon: Layers },
];

// Type for theme settings
interface OverlayThemeSettings {
  background_color: string;
  text_color: string;
  accent_color: string;
  font_family: string;
  font_size: number;
  border_radius: number;
  padding: number;
  show_background: boolean;
  animation_enabled: boolean;
}

export function Overlay() {
  const { settings, updateSettings } = useSettings();
  const { regenerateOverlay } = useOverlay();
  const delaySec = useDelaySec();
  const delayMode = useDelayMode();
  const isBuffering = useIsBuffering();
  const notify = useNotify();

  const [activeTab, setActiveTab] = useState<'preview' | 'customize' | 'install'>('preview');
  const [previewMode, setPreviewMode] = useState<'live' | 'delayed' | 'buffering'>('live');
  const [customCss, setCustomCss] = useState('');
  const [copied, setCopied] = useState(false);
  const [generating, setGenerating] = useState(false);
  const previewRef = useRef<HTMLIFrameElement>(null);

  const overlayPort = settings?.overlay_port ?? 8080;
  const overlayUrl = `http://localhost:${overlayPort}/overlay`;
  const wsUrl = `ws://localhost:${overlayPort}/overlay/ws`;

  const handleSaveSettings = async (newSettings: Partial<Settings>) => {
    try {
      if (!settings) {
        throw new Error('Settings have not loaded yet. Please try again.');
      }
      await updateSettings({ ...settings, ...newSettings });
      notify('success', 'Settings saved', 'Overlay settings updated');
    } catch (error) {
      notify('error', 'Failed to save', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleGenerate = async () => {
    setGenerating(true);
    try {
      await regenerateOverlay();
      notify('success', 'Overlay generated', 'Overlay files have been regenerated');
    } catch (error) {
      notify('error', 'Failed to generate', error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setGenerating(false);
    }
  };

  const handleCopyUrl = () => {
    navigator.clipboard.writeText(overlayUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    notify('success', 'Copied', 'Overlay URL copied to clipboard');
  };

  const handleOpenOverlay = () => {
    window.open(overlayUrl, '_blank', 'width=400,height=100');
  };

  const handlePreviewModeChange = (mode: 'live' | 'delayed' | 'buffering') => {
    setPreviewMode(mode);
    // Send to preview iframe
    if (previewRef.current?.contentWindow) {
      previewRef.current.contentWindow.postMessage({
        type: 'SET_PREVIEW_MODE',
        mode
      }, '*');
    }
  };

  const applyPreset = (presetId: string) => {
    const presets: Record<string, { overlay_theme: OverlayThemeSettings; overlay_port: number }> = {
      minimal: {
        overlay_theme: {
          background_color: 'rgba(0, 0, 0, 0.8)',
          text_color: '#ffffff',
          accent_color: '#22c55e',
          font_family: 'system-ui, -apple-system, sans-serif',
          font_size: 18,
          border_radius: 25,
          padding: 12,
          show_background: true,
          animation_enabled: false,
        },
        overlay_port: 8080,
      },
      modern: {
        overlay_theme: {
          background_color: 'rgba(0, 0, 0, 0.8)',
          text_color: '#ffffff',
          accent_color: '#f97316',
          font_family: 'system-ui, -apple-system, sans-serif',
          font_size: 24,
          border_radius: 25,
          padding: 12,
          show_background: true,
          animation_enabled: true,
        },
        overlay_port: 8080,
      },
      gaming: {
        overlay_theme: {
          background_color: 'rgba(0, 0, 0, 0.9)',
          text_color: '#ffffff',
          accent_color: '#00ff88',
          font_family: 'system-ui, -apple-system, sans-serif',
          font_size: 28,
          border_radius: 25,
          padding: 12,
          show_background: true,
          animation_enabled: true,
        },
        overlay_port: 8080,
      },
      broadcast: {
        overlay_theme: {
          background_color: 'rgba(0, 0, 0, 0.8)',
          text_color: '#ffffff',
          accent_color: '#ff6b00',
          font_family: 'system-ui, -apple-system, sans-serif',
          font_size: 20,
          border_radius: 25,
          padding: 12,
          show_background: true,
          animation_enabled: false,
        },
        overlay_port: 8080,
      },
    };

    const preset = presets[presetId];
    if (preset && settings) {
      handleSaveSettings({ overlay_theme: preset.overlay_theme, overlay_port: preset.overlay_port });
      notify('success', 'Preset applied', `${PRESETS.find(p => p.id === presetId)?.name} preset applied`);
    }
  };

  const generateOverlayHtml = () => {
    const safeCustomCss = customCss.replace(/<\/style/gi, '<\\/style');
    const theme = settings?.overlay_theme || {
      background_color: 'rgba(0, 0, 0, 0.8)',
      text_color: '#ffffff',
      accent_color: '#ff6b35',
      font_family: 'system-ui, -apple-system, sans-serif',
      font_size: 14,
      border_radius: 8,
      padding: 12,
      show_background: true,
      animation_enabled: true,
    };

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>InstantDelay Overlay</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      background: transparent;
      font-family: ${theme.font_family};
      display: flex;
      justify-content: center;
      align-items: flex-start;
      min-height: 100vh;
      padding: 20px;
      pointer-events: none;
    }
    .overlay {
      pointer-events: auto;
      display: flex;
      align-items: center;
      gap: 10px;
      padding: ${theme.padding}px ${theme.padding * 2}px;
      border-radius: ${theme.border_radius}px;
      backdrop-filter: blur(10px);
      background: ${theme.show_background ? theme.background_color : 'transparent'};
      border: 2px solid transparent;
      transition: all 0.3s ease;
    }
    .overlay.live {
      border-color: ${theme.accent_color};
      box-shadow: 0 0 20px ${theme.accent_color}80;
    }
    .overlay.delayed {
      border-color: #ef4444;
      box-shadow: 0 0 20px #ef444480;
    }
    .overlay.buffering {
      border-color: #f97316;
      box-shadow: 0 0 20px #f9731680;
    }
    .indicator {
      width: 12px;
      height: 12px;
      border-radius: 50%;
      flex-shrink: 0;
      animation: ${theme.animation_enabled ? 'pulse 1.5s ease-in-out infinite' : 'none'};
    }
    .indicator.live { background: ${theme.accent_color}; }
    .indicator.delayed { background: #ef4444; }
    .indicator.buffering { background: #f97316; animation: spin 1s linear infinite; }
    .text {
      color: ${theme.text_color};
      font-size: ${theme.font_size}px;
      font-weight: 600;
      letter-spacing: 0.5px;
      text-transform: uppercase;
      white-space: nowrap;
    }
    .delay-time {
      color: #ef4444;
      font-variant-numeric: tabular-nums;
      min-width: 3ch;
      text-align: right;
    }
    @keyframes pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.5; transform: scale(1.2); } }
    @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
    @keyframes slideIn { from { opacity: 0; transform: translateY(-20px); } to { opacity: 1; transform: translateY(0); } }
    .overlay { animation: slideIn 0.3s ease-out; }
  </style>
${safeCustomCss ? `    <style>${safeCustomCss}</style>` : ''}
</head>
<body>
  <div id="overlay" class="overlay live">
    <div id="indicator" class="indicator live"></div>
    <span id="status" class="text">LIVE</span>
    <span id="delay" class="delay-time">${delaySec}s</span>
  </div>
  <script>
    let ws = null;
    let reconnectAttempts = 0;
    const maxReconnect = 10;

    function connect() {
      ws = new WebSocket('${wsUrl}');

      ws.onopen = () => {
        reconnectAttempts = 0;
        console.log('Overlay WebSocket connected');
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          updateOverlay(data);
        } catch (e) {
          console.error('Failed to parse overlay message:', e);
        }
      };

      ws.onclose = () => {
        if (reconnectAttempts < maxReconnect) {
          reconnectAttempts++;
          setTimeout(connect, Math.min(1000 * Math.pow(1.5, reconnectAttempts), 30000));
        }
      };

      ws.onerror = (err) => {
        console.error('Overlay WebSocket error:', err);
      };
    }

    function updateOverlay(data) {
      const overlay = document.getElementById('overlay');
      const indicator = document.getElementById('indicator');
      const status = document.getElementById('status');
      const delayEl = document.getElementById('delay');

      if (data.mode === 'disabled') {
        overlay.className = 'overlay';
        indicator.className = 'indicator';
        status.textContent = 'NO DELAY';
        if (delayEl) delayEl.style.display = 'none';
        return;
      }

      if (data.buffering) {
        overlay.className = 'overlay buffering';
        indicator.className = 'indicator buffering';
        status.textContent = 'BUFFERING';
        if (delayEl) delayEl.style.display = 'none';
      } else if (data.mode === 'instant') {
        overlay.className = 'overlay delayed';
        indicator.className = 'indicator delayed';
        status.textContent = 'DELAYED';
        if (delayEl) {
          delayEl.textContent = data.delay_sec + 's';
          delayEl.style.display = 'inline';
        }
      } else {
        overlay.className = 'overlay live';
        indicator.className = 'indicator live';
        status.textContent = 'LIVE';
        if (delayEl) delayEl.style.display = 'none';
      }
    }

    // Listen for preview mode changes from parent
    window.addEventListener('message', (event) => {
      if (event.source !== window.parent || !event.data || event.data.type !== 'SET_PREVIEW_MODE') return;
      if (!['live', 'delayed', 'buffering'].includes(event.data.mode)) return;
      const mockData = {
        mode: event.data.mode === 'live' ? 'normal' : 'instant',
        buffering: event.data.mode === 'buffering',
        delay_sec: ${delaySec}
      };
      updateOverlay(mockData);
    });

    connect();
  <\/script>
</body></html>`;
  };

  const tabs = [
    { id: 'preview', label: 'Preview', icon: Monitor },
    { id: 'customize', label: 'Customize', icon: Palette },
    { id: 'install', label: 'Install', icon: Download },
  ];

  return (
    <div className="space-y-6 animate-in">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-surface-50">Overlay</h2>
          <p className="text-surface-400 text-sm mt-1">Configure the LIVE/DELAYED status overlay for OBS</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleGenerate}
            className="btn-secondary flex items-center gap-2"
            disabled={generating}
          >
            <RefreshCw className={clsx('w-4 h-4', generating && 'animate-spin')} />
            {generating ? 'Generating...' : 'Regenerate'}
          </button>
          <button
            onClick={handleOpenOverlay}
            className="btn-secondary flex items-center gap-2"
          >
            <ExternalLink className="w-4 h-4" />
            Open in Browser
          </button>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="card overflow-hidden">
        <div className="border-b border-surface-800">
          <nav className="flex" aria-label="Overlay tabs">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id as typeof activeTab)}
                  className={clsx(
                    'px-4 py-3 text-sm font-medium border-b-2 transition-colors flex items-center gap-2',
                    activeTab === tab.id
                      ? 'border-primary-500 text-primary-400'
                      : 'border-transparent text-surface-500 hover:text-surface-200 hover:border-surface-700'
                  )}
                  role="tab"
                  aria-selected={activeTab === tab.id}
                >
                  <Icon className="w-4 h-4" aria-hidden="true" />
                  {tab.label}
                </button>
              );
            })}
          </nav>
        </div>

        <div className="p-6">
          {/* Preview Tab */}
          {activeTab === 'preview' && (
            <div className="space-y-6">
              {/* Live Preview */}
              <div>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-surface-50">Live Preview</h3>
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-surface-500">Mode:</span>
                    <div className="flex gap-1 bg-surface-800 rounded-lg p-1">
                      {(['live', 'delayed', 'buffering'] as const).map((mode) => (
                        <button
                          key={mode}
                          onClick={() => handlePreviewModeChange(mode)}
                          className={clsx(
                            'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                            previewMode === mode
                              ? 'bg-primary-500 text-white'
                              : 'text-surface-400 hover:text-surface-200'
                          )}
                        >
                          {mode.charAt(0).toUpperCase() + mode.slice(1)}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>

                <div className="bg-surface-950 rounded-xl border border-surface-800 overflow-hidden">
                  <iframe
                    ref={previewRef}
                    srcDoc={generateOverlayHtml()}
                    className="w-full"
                    style={{ height: '120px', border: 'none' }}
                    title="Overlay preview"
                    sandbox="allow-scripts"
                  />
                </div>
              </div>

              {/* Current Status */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="card p-4">
                  <p className="text-sm text-surface-500 mb-1">Current Delay</p>
                  <p className="text-3xl font-bold font-mono text-primary-400">{delaySec}s</p>
                </div>
                <div className="card p-4">
                  <p className="text-sm text-surface-500 mb-1">Delay Mode</p>
                  <p className="text-3xl font-bold text-surface-50 capitalize">{delayMode}</p>
                </div>
                <div className="card p-4">
                  <p className="text-sm text-surface-500 mb-1">Buffer Status</p>
                  <p className="text-3xl font-bold capitalize">{isBuffering ? 'Buffering' : 'Ready'}</p>
                </div>
              </div>

              {/* OBS Browser Source Config */}
              <div className="card p-4">
                <h4 className="font-semibold text-surface-50 mb-3 flex items-center gap-2">
                  <Monitor className="w-5 h-5" />
                  OBS Browser Source Configuration
                </h4>
                <div className="space-y-3">
                  <div>
                    <label className="label">URL</label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={overlayUrl}
                        readOnly
                        className="input flex-1 font-mono"
                      />
                      <button
                        onClick={handleCopyUrl}
                        className={clsx(
                          'btn-secondary flex items-center gap-2 whitespace-nowrap',
                          copied && 'bg-green-500/20 border-green-500 text-green-400'
                        )}
                      >
                        {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                        {copied ? 'Copied!' : 'Copy'}
                      </button>
                    </div>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div>
                      <label className="label">Width</label>
                      <input type="number" value={320} readOnly className="input" />
                    </div>
                    <div>
                      <label className="label">Height</label>
                      <input type="number" value={100} readOnly className="input" />
                    </div>
                    <div>
                      <label className="label">FPS</label>
                      <input type="number" value={30} readOnly className="input" />
                    </div>
                    <div>
                      <label className="label">CSS</label>
                      <input type="text" value="body { background: transparent; }" readOnly className="input" />
                    </div>
                  </div>
                  <div className="p-3 bg-surface-900/50 rounded-lg border border-surface-800 text-sm text-surface-400">
                    <p className="font-medium text-surface-300 mb-1">How to add in OBS:</p>
                    <ol className="list-decimal list-inside space-y-1">
                      <li>Add a new <strong>Browser Source</strong> in your scene</li>
                      <li>Paste the URL above into the URL field</li>
                      <li>Set Width: 320, Height: 100 (or adjust as needed)</li>
                      <li>Check <strong>"Refresh browser when scene becomes active"</strong></li>
                      <li>Uncheck <strong>"Shutdown source when not visible"</strong> for instant updates</li>
                    </ol>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Customize Tab */}
          {activeTab === 'customize' && settings && (
            <div className="space-y-6">
              {/* Presets */}
              <div>
                <h3 className="settings-group-title">Presets</h3>
                <div className="flex flex-wrap gap-3">
                  {PRESETS.map((preset) => {
                    const Icon = preset.icon;
                    return (
                      <button
                        key={preset.id}
                        onClick={() => applyPreset(preset.id)}
                        className="btn-secondary flex items-center gap-2"
                      >
                        <Icon className="w-4 h-4" />
                        {preset.name}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Appearance Settings */}
              <div className="card p-6 space-y-6">
                <h3 className="settings-group-title">Appearance</h3>
                <div className="settings-group space-y-4">
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                    <div>
                      <label className="label">LIVE/Accent Color</label>
                      <div className="flex items-center gap-3">
                        <input
                          type="color"
                          value={settings.overlay_theme.accent_color}
                          onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, accent_color: e.target.value } })}
                          className="w-12 h-12 rounded border border-surface-700 cursor-pointer"
                        />
                        <input
                          type="text"
                          value={settings.overlay_theme.accent_color}
                          onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, accent_color: e.target.value } })}
                          className="input font-mono"
                        />
                      </div>
                    </div>
                    <div>
                      <label className="label">Background Color</label>
                      <div className="flex items-center gap-3">
                        <input
                          type="color"
                          value={settings.overlay_theme.background_color}
                          onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, background_color: e.target.value } })}
                          className="w-12 h-12 rounded border border-surface-700 cursor-pointer"
                        />
                        <input
                          type="text"
                          value={settings.overlay_theme.background_color}
                          onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, background_color: e.target.value } })}
                          className="input font-mono"
                        />
                      </div>
                    </div>
                  </div>

                  <div>
                    <label className="label">Font Size: {settings.overlay_theme.font_size}px</label>
                    <input
                      type="range"
                      min="12"
                      max="100"
                      value={settings.overlay_theme.font_size}
                      onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, font_size: parseInt(e.target.value) } })}
                      className="w-full h-2 bg-surface-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                    />
                  </div>

                  <div>
                    <label className="label">Border Radius: {settings.overlay_theme.border_radius}px</label>
                    <input
                      type="range"
                      min="0"
                      max="50"
                      value={settings.overlay_theme.border_radius}
                      onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, border_radius: parseInt(e.target.value) } })}
                      className="w-full h-2 bg-surface-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                    />
                  </div>

                  <div>
                    <label className="label">Padding: {settings.overlay_theme.padding}px</label>
                    <input
                      type="range"
                      min="4"
                      max="32"
                      value={settings.overlay_theme.padding}
                      onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, padding: parseInt(e.target.value) } })}
                      className="w-full h-2 bg-surface-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                    />
                  </div>

                  <div className="flex items-center justify-between">
                    <div>
                      <p className="label">Show Background</p>
                      <p className="text-sm text-surface-500">Display semi-transparent background</p>
                    </div>
                    <label className="custom-switch">
                      <input
                        type="checkbox"
                        checked={settings.overlay_theme.show_background}
                        onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, show_background: e.target.checked } })}
                      />
                      <span className="switch-track" />
                      <span className="switch-thumb" />
                    </label>
                  </div>

                  <div className="flex items-center justify-between">
                    <div>
                      <p className="label">Pulse Animation</p>
                      <p className="text-sm text-surface-500">Animate the status indicator</p>
                    </div>
                    <label className="custom-switch">
                      <input
                        type="checkbox"
                        checked={settings.overlay_theme.animation_enabled}
                        onChange={(e) => handleSaveSettings({ overlay_theme: { ...settings.overlay_theme, animation_enabled: e.target.checked } })}
                      />
                      <span className="switch-track" />
                      <span className="switch-thumb" />
                    </label>
                  </div>
                </div>
              </div>

              {/* Custom CSS */}
              <div className="card p-6">
                <h3 className="settings-group-title">Custom CSS</h3>
                <p className="text-sm text-surface-500 mb-4">Add custom CSS to further customize the overlay appearance.</p>
                <textarea
                  value={customCss}
                  onChange={(e) => setCustomCss(e.target.value)}
                  className="input font-mono min-h-[200px] bg-surface-950 resize-y"
                  placeholder="/* Custom CSS */\n.overlay {\n  border-radius: 100px;\n}"
                  spellCheck={false}
                />
                <div className="flex items-center justify-end gap-2 mt-3">
                  <button
                    onClick={() => {
                      handleSaveSettings({ customCss } as any);
                      notify('success', 'Custom CSS saved', 'Your custom styles have been applied');
                    }}
                    className="btn-primary"
                  >
                    Apply Custom CSS
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Install Tab */}
          {activeTab === 'install' && settings && (
            <div className="space-y-6">
              <div className="card p-6">
                <h3 className="settings-group-title">Quick Install</h3>
                <p className="text-surface-400 mb-6">Copy the URL below and paste it into OBS as a Browser Source.</p>

                <div className="space-y-4">
                  <div>
                    <label className="label">Overlay URL</label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={overlayUrl}
                        readOnly
                        className="input flex-1 font-mono text-lg"
                      />
                      <button
                        onClick={handleCopyUrl}
                        className={clsx(
                          'btn-primary flex items-center gap-2 whitespace-nowrap',
                          copied && 'bg-green-500 border-green-500'
                        )}
                      >
                        {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                        {copied ? 'Copied!' : 'Copy URL'}
                      </button>
                    </div>
                  </div>

                  <div className="p-4 bg-surface-900/50 rounded-lg border border-surface-800">
                    <h4 className="font-medium text-surface-300 mb-3 flex items-center gap-2">
                      <Monitor className="w-4 h-4" />
                      OBS Browser Source Settings
                    </h4>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
                      <div className="flex justify-between">
                        <span className="text-surface-500">URL</span>
                        <code className="text-surface-300 font-mono">{overlayUrl}</code>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-surface-500">Width</span>
                        <code className="text-surface-300">320</code>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-surface-500">Height</span>
                        <code className="text-surface-300">100</code>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-surface-500">FPS</span>
                        <code className="text-surface-300">30</code>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-surface-500">CSS</span>
                        <code className="text-surface-300">{'body { background: transparent; }'}</code>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <div className="card p-6">
                <h3 className="settings-group-title">Step-by-Step Guide</h3>
                <div className="space-y-4">
                  {[
                    { step: 1, title: 'Add Browser Source', desc: 'In OBS, right-click in Sources → Add → Browser Source' },
                    { step: 2, title: 'Configure URL', desc: 'Paste the overlay URL above into the URL field' },
                    { step: 3, title: 'Set Dimensions', desc: 'Width: 320, Height: 100 (adjust for your layout)' },
                    { step: 4, title: 'Enable Refresh', desc: 'Check "Refresh browser when scene becomes active"' },
                    { step: 5, title: 'Disable Shutdown', desc: 'Uncheck "Shutdown source when not visible" for real-time updates' },
                    { step: 6, title: 'Position', desc: 'Drag the source to your desired position in the preview' },
                  ].map((item) => (
                    <div key={item.step} className="flex items-start gap-4 p-4 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-primary-500/20 text-primary-400 flex items-center justify-center font-bold">
                        {item.step}
                      </div>
                      <div>
                        <p className="font-medium text-surface-100">{item.title}</p>
                        <p className="text-sm text-surface-500">{item.desc}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="card p-6">
                <h3 className="settings-group-title">Advanced Options</h3>
                <div className="space-y-3">
                  <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                    <div>
                      <p className="font-medium text-surface-300">WebSocket URL (for custom integration)</p>
                      <p className="text-sm text-surface-500 font-mono">{wsUrl}</p>
                    </div>
                    <button
                      onClick={() => navigator.clipboard.writeText(wsUrl)}
                      className="btn-ghost text-sm"
                    >
                      <Copy className="w-4 h-4" />
                    </button>
                  </div>
                  <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                    <div>
                      <p className="font-medium text-surface-300">REST API Endpoint</p>
                      <p className="text-sm text-surface-500 font-mono">http://localhost:{settings.web_ui_port}/api/overlay/status</p>
                    </div>
                    <button
                      onClick={() => navigator.clipboard.writeText(`http://localhost:${settings.web_ui_port}/api/overlay/status`)}
                      className="btn-ghost text-sm"
                    >
                      <Copy className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

