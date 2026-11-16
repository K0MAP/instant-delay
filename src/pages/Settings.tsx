import { useState } from 'react';
import {
  Server,
  HardDrive,
  Cpu,
  MemoryStick,
  Keyboard,
  RotateCcw,
  Save,
  RefreshCw,
  Info,
  Moon,
  Sun,
  Zap,
  Monitor,
  ExternalLink,
  Github,
  AlertCircle
} from 'lucide-react';
import { useSettings, useHotkeys, useSystem, useNotifications } from '../hooks/useTauri';
import { useTheme } from '../hooks/useTheme';
import type { Settings as SettingsType, HotkeyConfig } from '../types';
import clsx from 'clsx';
import { useTranslation } from '../i18n';

export function Settings() {
  const { t, language, setLanguage } = useTranslation();
  const { settings, updateSettings } = useSettings();
  const { hotkeys, updateHotkeyConfig } = useHotkeys();
  const { getSystemInfo, systemInfo } = useSystem();
  const { theme, setTheme } = useTheme();
  const { notify } = useNotifications();
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [activeTab, setActiveTab] = useState<'general' | 'server' | 'hotkeys' | 'overlay' | 'advanced' | 'about'>('general');

  const tabs = [
    { id: 'general', label: 'General', icon: Monitor },
    { id: 'server', label: 'RTMP Server', icon: Server },
    { id: 'hotkeys', label: 'Hotkeys', icon: Keyboard },
    { id: 'overlay', label: 'Overlay', icon: Monitor },
    { id: 'advanced', label: 'Advanced', icon: Cpu },
    { id: 'about', label: 'About', icon: Info },
  ];

  const handleSave = async (newSettings: Partial<SettingsType>) => {
    setSaving(true);
    try {
      await updateSettings({ ...settings, ...newSettings } as SettingsType);
      notify('success', 'Settings saved', 'Your changes have been applied');
    } catch (error) {
      notify('error', 'Failed to save settings', error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setSaving(false);
    }
  };

  const handleHotkeyChange = async (action: string, keys: string) => {
    try {
      const newHotkeys = { ...hotkeys, [action]: keys || undefined } as HotkeyConfig;
      await updateHotkeyConfig(newHotkeys);
      notify('success', 'Hotkey updated', `${action} set to ${keys}`);
    } catch (error) {
      notify('error', 'Failed to update hotkey', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleTestOverlay = async () => {
    setTesting(true);
    try {
      await import('@tauri-apps/api/core').then(({ invoke }) => invoke('regenerate_overlay'));
      notify('success', 'Overlay generated', 'Overlay files have been regenerated');
    } catch (error) {
      notify('error', 'Failed to generate overlay', error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setTesting(false);
    }
  };

  const handleOpenConfigFolder = async () => {
    try {
      await import('@tauri-apps/api/core').then(({ invoke }) => invoke('open_config_folder'));
    } catch (error) {
      notify('error', 'Failed to open folder', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const formatHotkey = (keys: string) => {
    return keys.split('+').map(k =>
      k === 'Control' ? 'Ctrl' :
      k === 'Alt' ? 'Alt' :
      k === 'Shift' ? 'Shift' :
      k === 'Meta' ? 'Cmd' : k
    ).join(' + ');
  };

  return (
    <div className="space-y-6 animate-in">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-surface-50">{t('Settings')}</h2>
          <p className="text-surface-400 text-sm mt-1">{t('Configure InstantDelay to match your workflow')}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleOpenConfigFolder}
            className="btn-secondary flex items-center gap-2 text-sm"
          >
            <ExternalLink className="w-3 h-3" />
            {t('Open Config Folder')}
          </button>
          <button
            onClick={() => { setSaving(true); handleSave(settings || {} as SettingsType); }}
            className="btn-primary flex items-center gap-2"
            disabled={saving || !settings}
          >
            <Save className="w-4 h-4" />
            {saving ? t('Saving...') : t('Save All')}
          </button>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="card overflow-hidden">
        <div className="border-b border-surface-800">
          <nav className="flex overflow-x-auto" aria-label={t('Settings tabs')}>
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id as typeof activeTab)}
                  className={clsx(
                    'px-4 py-3 text-sm font-medium border-b-2 transition-colors whitespace-nowrap flex items-center gap-2',
                    activeTab === tab.id
                      ? 'border-primary-500 text-primary-400'
                      : 'border-transparent text-surface-500 hover:text-surface-200 hover:border-surface-700'
                  )}
                  role="tab"
                  aria-selected={activeTab === tab.id}
                >
                  <Icon className="w-4 h-4" aria-hidden="true" />
                  {t(tab.label)}
                </button>
              );
            })}
          </nav>
        </div>

        {/* Tab Panels */}
        <div className="p-6">
          {/* General Tab */}
          {activeTab === 'general' && settings && (
            <div className="settings-section">
              <h3 className="settings-group-title">{t('Application')}</h3>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">{t('Theme')}</label>
                  <div className="flex gap-2">
                    {(['dark', 'light', 'system'] as const).map((t) => (
                      <button
                        key={t}
                        onClick={() => setTheme(t)}
                        className={clsx(
                          'flex-1 py-2 px-4 rounded-lg border-2 transition-colors flex items-center justify-center gap-2',
                          theme === t
                            ? 'border-primary-500 bg-primary-500/10 text-primary-400'
                            : 'border-surface-700 text-surface-300 hover:border-surface-600'
                        )}
                      >
                        {t === 'dark' && <Moon className="w-4 h-4" />}
                        {t === 'light' && <Sun className="w-4 h-4" />}
                        {t === 'system' && <Monitor className="w-4 h-4" />}
                        {language === 'uk' ? (t === 'dark' ? 'Темна' : t === 'light' ? 'Світла' : 'Системна') : t.charAt(0).toUpperCase() + t.slice(1)}
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <label className="label" htmlFor="app-language">{t('Language')}</label>
                  <select
                    id="app-language"
                    value={language}
                    onChange={(event) => setLanguage(event.target.value as 'en' | 'uk')}
                    className="input mt-2 w-full"
                  >
                    <option value="en">{t('English')}</option>
                    <option value="uk">{t('Ukrainian')}</option>
                  </select>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">{t('Auto-start server')}</p>
                    <p className="text-sm text-surface-500">{t('Start RTMP server on application launch')}</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={settings.auto_start_server}
                      onChange={(e) => handleSave({ auto_start_server: e.target.checked })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">{t('Minimize to tray')}</p>
                    <p className="text-sm text-surface-500">{t('Hide window to system tray on close')}</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={settings.minimize_to_tray}
                      onChange={(e) => handleSave({ minimize_to_tray: e.target.checked })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">{t('Show notifications')}</p>
                    <p className="text-sm text-surface-500">{t('Display desktop notifications')}</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={true} // Not in settings, using default
                      onChange={(e) => notify('info', 'Notifications', e.target.checked ? 'Enabled' : 'Disabled')}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">{t('Check for updates')}</p>
                    <p className="text-sm text-surface-500">{t('Automatically check for new versions')}</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={settings.check_updates}
                      onChange={(e) => handleSave({ check_updates: e.target.checked })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>
              </div>

              <h3 className="settings-group-title">Stream Deck</h3>
              <div className="settings-group space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">Enable Stream Deck plugin</p>
                    <p className="text-sm text-surface-500">Allow Stream Deck integration</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={settings.streamdeck_enabled}
                      onChange={(e) => handleSave({ streamdeck_enabled: e.target.checked })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div>
                  <label className="label">Stream Deck Port</label>
                  <input
                    type="number"
                    value={settings.streamdeck_port}
                    onChange={(e) => handleSave({ streamdeck_port: parseInt(e.target.value) })}
                    className="input"
                    min="1024"
                    max="65535"
                  />
                </div>
              </div>
            </div>
          )}

          {/* RTMP Server Tab */}
          {activeTab === 'server' && settings && (
            <div className="settings-section">
              <h3 className="settings-group-title">RTMP Server</h3>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">Bind Address</label>
                  <input
                    type="text"
                    value={settings.bind_address}
                    onChange={(e) => handleSave({ bind_address: e.target.value })}
                    className="input font-mono"
                    placeholder="127.0.0.1"
                  />
                  <p className="text-xs text-surface-500 mt-1">IP address to bind the RTMP server to</p>
                </div>

                <div>
                  <label className="label">Port</label>
                  <input
                    type="number"
                    value={settings.port}
                    onChange={(e) => handleSave({ port: parseInt(e.target.value) })}
                    className="input"
                    min="1"
                    max="65535"
                  />
                  <p className="text-xs text-surface-500 mt-1">Default RTMP port is 1935</p>
                </div>

                <div>
                  <label className="label">Max Connections</label>
                  <input
                    type="number"
                    value={10} // Not in settings, using default
                    onChange={(e) => notify('info', 'Max Connections', `Set to ${e.target.value}`)}
                    className="input"
                    min="1"
                    max="100"
                  />
                </div>

                <div>
                  <label className="label">Chunk Size</label>
                  <input
                    type="number"
                    value={4096} // Not in settings, using default
                    onChange={(e) => notify('info', 'Chunk Size', `Set to ${e.target.value}`)}
                    className="input"
                    min="128"
                    max="65536"
                    step="128"
                  />
                  <p className="text-xs text-surface-500 mt-1">RTMP chunk size in bytes (default: 4096)</p>
                </div>
              </div>

              <h3 className="settings-group-title">Buffer Settings</h3>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">Default Delay (seconds)</label>
                  <input
                    type="number"
                    value={settings.default_delay_sec}
                    onChange={(e) => handleSave({ default_delay_sec: parseInt(e.target.value) })}
                    className="input"
                    min="1"
                    max="600"
                  />
                </div>

                <div>
                  <label className="label">Max Delay (seconds)</label>
                  <input
                    type="number"
                    value={settings.max_delay_sec}
                    onChange={(e) => handleSave({ max_delay_sec: parseInt(e.target.value) })}
                    className="input"
                    min="1"
                    max="3600"
                  />
                </div>

                <div>
                  <label className="label">Buffer Size (MB)</label>
                  <input
                    type="number"
                    value={settings.buffer_max_memory_mb}
                    onChange={(e) => handleSave({ buffer_max_memory_mb: parseInt(e.target.value) })}
                    className="input"
                    min="10"
                    max="2048"
                  />
                  <p className="text-xs text-surface-500 mt-1">Maximum ring buffer size in megabytes</p>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">Pre-buffer on connect</p>
                    <p className="text-sm text-surface-500">Start buffering immediately when OBS connects</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={!settings.buffer_keyframe_only} // Inverse of buffer_keyframe_only
                      onChange={(e) => handleSave({ buffer_keyframe_only: !e.target.checked })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>
              </div>
            </div>
          )}

          {/* Hotkeys Tab */}
          {activeTab === 'hotkeys' && (
            <div className="settings-section">
              <h3 className="settings-group-title">Global Hotkeys</h3>
              <p className="text-surface-500 text-sm mb-4">
                Configure global keyboard shortcuts that work even when InstantDelay is not focused.
                Press the key combination you want to assign.
              </p>
              <div className="settings-group space-y-3">
                {[
                  { action: 'toggle_delay', label: 'Toggle Delay', description: 'Enable/disable stream delay' },
                  { action: 'increase_delay', label: 'Increase Delay', description: 'Add 5 seconds to delay' },
                  { action: 'decrease_delay', label: 'Decrease Delay', description: 'Subtract 5 seconds from delay' },
                  { action: 'skip_to_live', label: 'Skip to Live', description: 'Jump to live position' },
                  { action: 'toggle_video', label: 'Toggle Video', description: 'Toggle video output' },
                  { action: 'toggle_audio', label: 'Toggle Audio', description: 'Toggle audio output' },
                ].map(({ action, label, description }) => {
                  const hotkey = hotkeys[action as keyof HotkeyConfig];
                  const [recording, setRecording] = useState<string | null>(null);

                  return (
                    <div key={action} className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex-1 min-w-0">
                        <p className="font-medium text-surface-100">{label}</p>
                        <p className="text-sm text-surface-500">{description}</p>
                      </div>
                      <div className="flex items-center gap-2">
                        {recording === action ? (
                          <div className="flex items-center gap-2">
                            <kbd className="px-3 py-1.5 bg-primary-500/20 border border-primary-500 rounded text-primary-400 font-mono text-sm">
                              Press keys...
                            </kbd>
                            <button
                              onClick={() => setRecording(null)}
                              className="btn-ghost text-sm"
                            >
                              Cancel
                            </button>
                          </div>
                        ) : (
                          <>
                            <kbd className="px-3 py-1.5 bg-surface-700 border border-surface-600 rounded text-surface-300 font-mono text-sm min-w-[160px] text-center">
                              {hotkey ? formatHotkey(hotkey) : 'Not set'}
                            </kbd>
                            <button
                              onClick={() => setRecording(action)}
                              className="btn-secondary text-sm"
                              disabled={recording !== null}
                            >
                              {recording ? 'Recording...' : 'Change'}
                            </button>
                            {hotkey && (
                              <button
                                onClick={() => handleHotkeyChange(action, '')}
                                className="btn-ghost text-sm text-surface-500 hover:text-red-400"
                                aria-label="Clear hotkey"
                              >
                                <RotateCcw className="w-4 h-4" />
                              </button>
                            )}
                          </>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Overlay Tab */}
          {activeTab === 'overlay' && settings && (
            <div className="settings-section">
              <h3 className="settings-group-title">LIVE/DELAYED Overlay</h3>
              <p className="text-surface-500 text-sm mb-4">
                Configure the browser source overlay for OBS. The overlay shows viewers whether the stream is LIVE or DELAYED.
              </p>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">Overlay Port</label>
                  <input
                    type="number"
                    value={settings.overlay_port}
                    onChange={(e) => handleSave({ overlay_port: parseInt(e.target.value) })}
                    className="input"
                    min="1024"
                    max="65535"
                  />
                  <p className="text-xs text-surface-500 mt-1">Port for overlay WebSocket server (default: 8080)</p>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label className="label">LIVE Color</label>
                    <div className="flex items-center gap-3">
                      <input
                        type="color"
                        value={settings.overlay_theme.accent_color}
                        onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, accent_color: e.target.value } })}
                        className="w-10 h-10 rounded border border-surface-700 cursor-pointer"
                      />
                      <input
                        type="text"
                        value={settings.overlay_theme.accent_color}
                        onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, accent_color: e.target.value } })}
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
                        onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, background_color: e.target.value } })}
                        className="w-10 h-10 rounded border border-surface-700 cursor-pointer"
                      />
                      <input
                        type="text"
                        value={settings.overlay_theme.background_color}
                        onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, background_color: e.target.value } })}
                        className="input font-mono"
                      />
                    </div>
                  </div>
                </div>

                <div>
                  <label className="label">Font Size (px)</label>
                  <input
                    type="number"
                    value={settings.overlay_theme.font_size}
                    onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, font_size: parseInt(e.target.value) } })}
                    className="input"
                    min="12"
                    max="100"
                  />
                </div>

                <div>
                  <label className="label">Border Radius (px)</label>
                  <input
                    type="number"
                    value={settings.overlay_theme.border_radius}
                    onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, border_radius: parseInt(e.target.value) } })}
                    className="input"
                    min="0"
                    max="50"
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
                      onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, show_background: e.target.checked } })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">Animation Enabled</p>
                    <p className="text-sm text-surface-500">Animate the status indicator</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={settings.overlay_theme.animation_enabled}
                      onChange={(e) => handleSave({ overlay_theme: { ...settings.overlay_theme, animation_enabled: e.target.checked } })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <button
                  onClick={handleTestOverlay}
                  className="btn-secondary flex items-center gap-2"
                  disabled={testing}
                >
                  <RefreshCw className={clsx('w-4 h-4', testing && 'animate-spin')} />
                  {testing ? 'Regenerating...' : 'Regenerate Overlay Files'}
                </button>

                <div className="p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                  <p className="text-sm text-surface-400 mb-2">OBS Browser Source URL:</p>
                  <code className="text-surface-300 font-mono text-sm break-all">
                    http://localhost:{settings.overlay_port}/overlay
                  </code>
                </div>
              </div>
            </div>
          )}

          {/* Advanced Tab */}
          {activeTab === 'advanced' && settings && (
            <div className="settings-section">
              <h3 className="settings-group-title">Web Server</h3>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">HTTP Port</label>
                  <input
                    type="number"
                    value={settings.web_ui_port}
                    onChange={(e) => handleSave({ web_ui_port: parseInt(e.target.value) })}
                    className="input"
                    min="1024"
                    max="65535"
                  />
                  <p className="text-xs text-surface-500 mt-1">Port for REST API and WebSocket (default: 8081)</p>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">Enable CORS</p>
                    <p className="text-sm text-surface-500">Allow cross-origin requests to API</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={true} // Not in settings
                      onChange={(e) => notify('info', 'CORS', e.target.checked ? 'Enabled' : 'Disabled')}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">API Authentication</p>
                    <p className="text-sm text-surface-500">Require API key for REST endpoints</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={!!settings.web_ui_auth_token}
                      onChange={(e) => handleSave({ web_ui_auth_token: e.target.checked ? 'auto-generated' : undefined })}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>
              </div>

              <h3 className="settings-group-title">Performance</h3>
              <div className="settings-group space-y-4">
                <div>
                  <label className="label">Log Level</label>
                  <select
                    value={settings.log_level}
                    onChange={(e) => handleSave({ log_level: e.target.value })}
                    className="input"
                  >
                    <option value="trace">Trace</option>
                    <option value="debug">Debug</option>
                    <option value="info">Info</option>
                    <option value="warn">Warn</option>
                    <option value="error">Error</option>
                  </select>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="label">Hardware acceleration</p>
                    <p className="text-sm text-surface-500">Use GPU for video processing (experimental)</p>
                  </div>
                  <label className="custom-switch">
                    <input
                      type="checkbox"
                      checked={false} // Not in settings
                      onChange={(e) => notify('info', 'Hardware Acceleration', e.target.checked ? 'Enabled' : 'Disabled')}
                    />
                    <span className="switch-track" />
                    <span className="switch-thumb" />
                  </label>
                </div>
              </div>

              <h3 className="settings-group-title">System Information</h3>
              <div className="settings-group space-y-3">
                {systemInfo && (
                  <>
                    <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex items-center gap-3">
                        <Cpu className="w-5 h-5 text-surface-500" />
                        <div>
                          <p className="text-sm text-surface-400">CPU Cores</p>
                          <p className="font-mono text-surface-300">{systemInfo.cpu_cores}</p>
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex items-center gap-3">
                        <MemoryStick className="w-5 h-5 text-surface-500" />
                        <div>
                          <p className="text-sm text-surface-400">Total Memory</p>
                          <p className="font-mono text-surface-300">{systemInfo.total_memory_mb} MB</p>
                        </div>
                      </div>
                      <span className="text-sm text-surface-500">
                        {systemInfo.available_memory_mb} MB available
                      </span>
                    </div>
                    <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex items-center gap-3">
                        <HardDrive className="w-5 h-5 text-surface-500" />
                        <div>
                          <p className="text-sm text-surface-400">OS</p>
                          <p className="font-mono text-surface-300">{systemInfo.os} {systemInfo.arch}</p>
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center justify-between p-3 bg-surface-900/50 rounded-lg border border-surface-800">
                      <div className="flex items-center gap-3">
                        <Monitor className="w-5 h-5 text-surface-500" />
                        <div>
                          <p className="text-sm text-surface-400">OBS</p>
                          <p className="font-mono text-surface-300">{systemInfo.obs_running ? 'Running' : 'Not running'}</p>
                        </div>
                      </div>
                      {systemInfo.obs_version && (
                        <span className="text-sm text-surface-500">v{systemInfo.obs_version}</span>
                      )}
                    </div>
                  </>
                )}
                <button
                  onClick={() => getSystemInfo()}
                  className="btn-secondary flex items-center gap-2"
                >
                  <RefreshCw className="w-4 h-4" />
                  Refresh System Info
                </button>
              </div>
            </div>
          )}

          {/* About Tab */}
          {activeTab === 'about' && (
            <div className="settings-section">
              <div className="text-center py-12">
                <div className="w-24 h-24 rounded-2xl bg-gradient-to-br from-primary-500 to-primary-600 flex items-center justify-center mx-auto mb-6">
                  <Zap className="w-12 h-12 text-white" />
                </div>
                <h3 className="text-2xl font-bold text-surface-50 mb-2">InstantDelay</h3>
                <p className="text-surface-400 mb-1">Version 1.0.0</p>
                <p className="text-surface-500 text-sm mt-4 max-w-md mx-auto">
                  Experimental stream delay control for a local RTMP input and configured output destinations.
                </p>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-8">
                <div className="card p-4 text-center">
                  <Github className="w-8 h-8 mx-auto mb-2 text-surface-400" />
                  <p className="font-medium text-surface-300">Open Source</p>
                  <p className="text-sm text-surface-500">MIT Licensed</p>
                </div>
                <div className="card p-4 text-center">
                  <Server className="w-8 h-8 mx-auto mb-2 text-surface-400" />
                  <p className="font-medium text-surface-300">Rust + Tauri</p>
                  <p className="text-sm text-surface-500">Native Performance</p>
                </div>
                <div className="card p-4 text-center">
                  <Monitor className="w-8 h-8 mx-auto mb-2 text-surface-400" />
                  <p className="font-medium text-surface-300">React + TypeScript</p>
                  <p className="text-sm text-surface-500">Modern UI</p>
                </div>
              </div>

              <div className="card p-6">
                <h4 className="font-semibold text-surface-50 mb-4">Links</h4>
                <div className="flex flex-wrap gap-4">
                  <a href="https://github.com/K0MAP/instant-delay" target="_blank" rel="noopener noreferrer" className="btn-secondary flex items-center gap-2">
                    <Github className="w-4 h-4" />
                    GitHub Repository
                  </a>
                  <a href="https://github.com/K0MAP/instant-delay/issues" target="_blank" rel="noopener noreferrer" className="btn-secondary flex items-center gap-2">
                    <AlertCircle className="w-4 h-4" />
                    Report Issue
                  </a>
                  <a href="https://github.com/K0MAP/instant-delay/releases" target="_blank" rel="noopener noreferrer" className="btn-secondary flex items-center gap-2">
                    <RefreshCw className="w-4 h-4" />
                    Releases
                  </a>
                </div>
              </div>

              <div className="card p-6">
                <h4 className="font-semibold text-surface-50 mb-4">Third Party Licenses</h4>
                <div className="space-y-2 text-sm text-surface-400">
                  <p>Tauri - MIT/Apache-2.0</p>
                  <p>React - MIT</p>
                  <p>Tokio - MIT</p>
                  <p>Axum - MIT</p>
                  <p>Zustand - MIT</p>
                  <p>Tailwind CSS - MIT</p>
                  <p>Lucide Icons - ISC</p>
                  <p>rdev - MIT/Apache-2.0</p>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

