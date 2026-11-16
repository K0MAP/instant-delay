import { useState } from 'react';
import {
  Plus,
  Trash2,
  Edit,
  Wifi,
  WifiOff,
  CheckCircle,
  Loader2,
  Copy,
  Eye,
  EyeOff,
  TestTube,
  Server,
  Globe,
  Youtube,
  Music,
  Send,
  Radio,
  X
} from 'lucide-react';
import { usePlatforms, useNotifications } from '../hooks/useTauri';
import { PlatformOutput, Platform } from '../types';
import clsx from 'clsx';
import { useTranslation } from '../i18n';

const PLATFORM_CONFIGS: Record<string, { icon: React.ComponentType<{className?: string}>; color: string; name: string }> = {
  twitch: { icon: Wifi, color: 'bg-purple-600', name: 'Twitch' },
  youtube: { icon: Youtube, color: 'bg-red-600', name: 'YouTube' },
  kick: { icon: Send, color: 'bg-green-600', name: 'Kick' },
  tiktok: { icon: Music, color: 'bg-pink-600', name: 'TikTok' },
  restream: { icon: Radio, color: 'bg-blue-600', name: 'Restream' },
  custom: { icon: Globe, color: 'bg-surface-600', name: 'Custom RTMP' },
};

interface PlatformFormData {
  name: string;
  platform: Platform;
  url: string;
  stream_key: string;
  enabled: boolean;
}

export function Platforms() {
  const { t } = useTranslation();
  const { platforms, addPlatformOutput, removePlatformOutput, updatePlatformOutput, testConnection } = usePlatforms();
  const { notify } = useNotifications();
  const [showModal, setShowModal] = useState(false);
  const [editingPlatform, setEditingPlatform] = useState<PlatformOutput | null>(null);
  const [formData, setFormData] = useState<PlatformFormData>({
    name: '',
    platform: 'custom',
    url: '',
    stream_key: '',
    enabled: true,
  });
  const [testing, setTesting] = useState<string | null>(null);
const isTesting = testing !== null;
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
const [showModalKey, setShowModalKey] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      if (editingPlatform) {
        const updatedPlatform = { ...editingPlatform, ...formData };
        await updatePlatformOutput(updatedPlatform);
        notify('success', 'Platform updated', `${formData.name} has been updated`);
      } else {
        await addPlatformOutput({
          ...formData,
          id: crypto.randomUUID(),
          priority: 0,
          backup: false,
        });
        notify('success', 'Platform added', `${formData.name} has been added`);
      }
      setShowModal(false);
      resetForm();
    } catch (error) {
      notify('error', 'Failed to save platform', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleEdit = (platform: PlatformOutput) => {
    setEditingPlatform(platform);
    setFormData({
      name: platform.name,
      platform: platform.platform,
      url: platform.url,
      stream_key: platform.stream_key,
      enabled: platform.enabled,
    });
    setShowModal(true);
  };

  const handleRemove = async (id: string, name: string) => {
    if (window.confirm(`Remove "${name}"? This cannot be undone.`)) {
      try {
        await removePlatformOutput(id);
        notify('success', 'Platform removed', `${name} has been removed`);
      } catch (error) {
        notify('error', 'Failed to remove platform', error instanceof Error ? error.message : 'Unknown error');
      }
    }
  };

  const handleTest = async (platform: PlatformOutput) => {
    setTesting(platform.id);
    try {
      const connected = await testConnection(platform);
      if (!connected) {
        throw new Error(`Could not connect to ${platform.name}`);
      }
      notify('success', 'Connection test passed', `Successfully connected to ${platform.name}`);
    } catch (error) {
      notify('error', 'Connection test failed', error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setTesting(null);
    }
  };

  const handleToggleEnabled = async (platform: PlatformOutput) => {
    try {
      const updatedPlatform = { ...platform, enabled: !platform.enabled };
      await updatePlatformOutput(updatedPlatform);
      notify('info', platform.enabled ? 'Platform disabled' : 'Platform enabled', `${platform.name} ${platform.enabled ? 'disabled' : 'enabled'}`);
    } catch (error) {
      notify('error', 'Failed to toggle platform', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleCopyKey = (key: string, name: string) => {
    navigator.clipboard.writeText(key);
    notify('success', 'Copied', `Stream key for ${name} copied to clipboard`);
  };

  const resetForm = () => {
    setFormData({ name: '', platform: 'custom', url: '', stream_key: '', enabled: true });
    setEditingPlatform(null);
    setShowModalKey(false);
  };

  const handlePlatformChange = (platform: string) => {
    const urls: Record<string, string> = {
      twitch: 'rtmp://live.twitch.tv/app/',
      youtube: 'rtmp://a.rtmp.youtube.com/live2/',
      kick: 'rtmp://ingest.kick.com/app/',
      tiktok: 'rtmp://push.tiktokcdn.com/live/',
      restream: 'rtmp://live.restream.io/live/',
      custom: '',
    };
    const platformValue = platform as Platform;
    setFormData(prev => ({ ...prev, platform: platformValue, url: urls[platform] || '' }));
  };

  return (
    <div className="space-y-6 animate-in">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-surface-50">{t('Platforms')}</h2>
          <p className="text-surface-400 text-sm mt-1">{t('Manage streaming destinations')}</p>
        </div>
        <button
          onClick={() => { resetForm(); setShowModal(true); }}
          className="btn-primary flex items-center gap-2"
        >
          <Plus className="w-4 h-4" />
          {t('Add Platform')}
        </button>
      </div>

      {/* Platform Cards */}
      {platforms.length === 0 ? (
        <div className="card p-12 empty-state">
          <Server className="empty-state-icon w-16 h-16" />
          <h3 className="empty-state-title">{t('No platforms configured')}</h3>
          <p className="empty-state-description">
            {t('Add your streaming destinations to start relaying your delayed stream to multiple platforms simultaneously.')}
          </p>
          <button
            onClick={() => { resetForm(); setShowModal(true); }}
            className="btn-primary mt-4 flex items-center gap-2"
          >
            <Plus className="w-4 h-4" />
            {t('Add Your First Platform')}
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {platforms.map((platform) => {
            const config = PLATFORM_CONFIGS[platform.platform] || PLATFORM_CONFIGS.custom;
            const PlatformIcon = config.icon;
            // PlatformOutput doesn't have connected/bytes_sent/uptime_sec - those are in PlatformOutputStatus
            // Use optional chaining with defaults
            const isConnected = (platform as any).connected ?? false;
            const bytesSent = (platform as any).bytes_sent ?? 0;
            const uptimeSec = (platform as any).uptime_sec ?? 0;

            return (
              <div key={platform.id} className="platform-card">
                <div className="platform-info">
                  <div className={clsx('platform-avatar', config.color)}>
                    <PlatformIcon className="w-5 h-5" aria-hidden="true" />
                  </div>
                  <div className="min-w-0">
                    <p className="platform-name">{platform.name}</p>
                    <p className="platform-platform">{config.name}</p>
                  </div>
                </div>

                <div className="platform-status flex items-center gap-3">
                  {/* Status indicator */}
                  <div className="flex items-center gap-2">
                    <div className={clsx(
                      'w-2.5 h-2.5 rounded-full',
                      isConnected && 'status-dot-live',
                      !isConnected && 'status-dot-offline'
                    )} />
                    <span className={clsx(
                      'text-xs font-medium',
                      isConnected && 'text-green-400',
                      !isConnected && 'text-surface-500'
                    )}>
                      {isConnected ? t('Live') : t('Offline')}
                    </span>
                  </div>

                  {/* Stats */}
                  {isConnected && (
                    <div className="hidden sm:flex items-center gap-4 text-xs text-surface-500">
                      <span>{bytesSent ? `${(bytesSent / 1024 / 1024).toFixed(1)} MB sent` : '0 MB sent'}</span>
                      <span>{uptimeSec ? `${Math.floor(uptimeSec / 60)}m` : '0m'}</span>
                    </div>
                  )}

                  {/* Actions */}
                  <div className="flex items-center gap-1">
                    {/* Toggle enabled */}
                    <button
                      onClick={() => handleToggleEnabled(platform)}
                      className={clsx(
                        'p-2 rounded-lg transition-colors',
                        platform.enabled
                          ? 'bg-green-500/10 text-green-400 hover:bg-green-500/20'
                          : 'bg-surface-700 text-surface-500 hover:bg-surface-600'
                      )}
                      aria-label={platform.enabled ? 'Disable platform' : 'Enable platform'}
                      disabled={testing === platform.id}
                    >
                      {platform.enabled ? <CheckCircle className="w-4 h-4" /> : <WifiOff className="w-4 h-4" />}
                    </button>

                    {/* Test connection */}
                    <button
                      onClick={() => handleTest(platform)}
                      className={clsx(
                        'p-2 rounded-lg bg-surface-700 text-surface-300 hover:bg-surface-600 transition-colors',
                        testing === platform.id && 'bg-primary-500/20 text-primary-400'
                      )}
                      aria-label="Test connection"
                      disabled={testing === platform.id || !platform.enabled}
                    >
                      {testing === platform.id ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <TestTube className="w-4 h-4" />
                      )}
                    </button>

                    {/* Edit */}
                    <button
                      onClick={() => handleEdit(platform)}
                      className="p-2 rounded-lg bg-surface-700 text-surface-300 hover:bg-surface-600 transition-colors"
                      aria-label="Edit platform"
                    >
                      <Edit className="w-4 h-4" />
                    </button>

                    {/* Delete */}
                    <button
                      onClick={() => handleRemove(platform.id, platform.name)}
                      className="p-2 rounded-lg bg-surface-700 text-surface-300 hover:bg-red-500/20 hover:text-red-400 transition-colors"
                      aria-label="Delete platform"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {/* Details expandable */}
                <details className="mt-4 w-full">
                  <summary className="flex items-center justify-between cursor-pointer text-sm text-surface-500 hover:text-surface-300">
                    <span>{t('Connection Details')}</span>
                    <Eye className="w-4 h-4 transition-transform duration-200" />
                  </summary>
                  <div className="mt-3 p-3 bg-surface-900/50 rounded-lg border border-surface-800 space-y-2 text-sm">
                    <div className="flex items-center justify-between gap-4 flex-wrap">
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <span className="text-surface-500 min-w-[80px]">RTMP URL:</span>
                        <code className="text-surface-300 font-mono truncate flex-1">{platform.url}</code>
                        <button
                          onClick={() => navigator.clipboard.writeText(platform.url)}
                          className="p-1 text-surface-500 hover:text-surface-200"
                          aria-label="Copy RTMP URL"
                        >
                          <Copy className="w-3 h-3" />
                        </button>
                      </div>
                    </div>
                    <div className="flex items-center justify-between gap-4 flex-wrap">
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <span className="text-surface-500 min-w-[80px]">{t('Stream Key')}:</span>
                        <code className="text-surface-300 font-mono truncate flex-1">
                          {showKeys[platform.id] ? platform.stream_key : '••••••••••••••••'}
                        </code>
                        <button
                          onClick={() => setShowKeys(prev => ({ ...prev, [platform.id]: !prev[platform.id] }))}
                          className="p-1 text-surface-500 hover:text-surface-200"
                          aria-label={showKeys[platform.id] ? 'Hide stream key' : 'Show stream key'}
                        >
                          {showKeys[platform.id] ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                        </button>
                        <button
                          onClick={() => handleCopyKey(platform.stream_key, platform.name)}
                          className="p-1 text-surface-500 hover:text-surface-200"
                          aria-label="Copy stream key"
                        >
                          <Copy className="w-3 h-3" />
                        </button>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-surface-500 min-w-[80px]">Status:</span>
                      <span className={clsx(
                        'px-2 py-0.5 rounded text-xs font-medium',
                        isConnected && 'bg-green-500/20 text-green-400',
                        !isConnected && 'bg-surface-700 text-surface-500'
                      )}>
                        {isConnected ? t('Connected') : t('Disconnected')}
                      </span>
                    </div>
                  </div>
                </details>
              </div>
            );
          })}
        </div>
      )}

      {/* Add/Edit Platform Modal */}
      {showModal && (
        <div className="modal-overlay" onClick={() => { setShowModal(false); resetForm(); }}>
          <div className="modal-content" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">{editingPlatform ? t('Edit Platform') : t('Add Platform')}</h3>
              <button
                onClick={() => { setShowModal(false); resetForm(); }}
                className="p-1 text-surface-400 hover:text-surface-200"
                aria-label="Close"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
            <form onSubmit={handleSubmit} className="modal-body space-y-4">
              <div>
                <label htmlFor="platform-name" className="label">Platform Name</label>
                <input
                  type="text"
                  id="platform-name"
                  value={formData.name}
                  onChange={(e) => setFormData(prev => ({ ...prev, name: e.target.value }))}
                  className="input"
                  placeholder="My Twitch Stream"
                  required
                  disabled={isTesting}
                />
              </div>

              <div>
                <label htmlFor="platform-type" className="label">Platform</label>
                <select
                  id="platform-type"
                  value={formData.platform}
                  onChange={(e) => handlePlatformChange(e.target.value as Platform)}
                  className="input"
                  disabled={isTesting}
                >
                  <option value="twitch">Twitch</option>
                  <option value="youtube">YouTube</option>
                  <option value="kick">Kick</option>
                  <option value="tiktok">TikTok</option>
                  <option value="restream">Restream.io</option>
                  <option value="custom">Custom RTMP</option>
                </select>
              </div>

              <div>
                <label htmlFor="rtmp-url" className="label">RTMP URL</label>
                <input
                  type="text"
                  id="rtmp-url"
                  value={formData.url}
                  onChange={(e) => setFormData(prev => ({ ...prev, url: e.target.value }))}
                  className="input"
                  placeholder="rtmp://server/app/"
                  required
                  disabled={isTesting}
                />
              </div>

              <div>
                <label htmlFor="stream-key" className="label">Stream Key</label>
                <div className="relative">
                  <input
                    type={showModalKey ? 'text' : 'password'}
                    id="stream-key"
                    value={formData.stream_key}
                    onChange={(e) => setFormData(prev => ({ ...prev, stream_key: e.target.value }))}
                    className="input pr-12"
                    placeholder="Your stream key"
                    required
                    disabled={isTesting}
                  />
                  <button
                    type="button"
                    onClick={() => setShowModalKey(prev => !prev)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-surface-500 hover:text-surface-200"
                    aria-label={showModalKey ? 'Hide stream key' : 'Show stream key'}
                  >
                    {showModalKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>
                <p className="text-xs text-surface-500 mt-1">Your stream key is stored locally and never transmitted</p>
              </div>

              <div className="flex items-center justify-between">
                <label className="custom-switch flex-1">
                  <input
                    type="checkbox"
                    checked={formData.enabled}
                    onChange={(e) => setFormData(prev => ({ ...prev, enabled: e.target.checked }))}
                    disabled={isTesting}
                  />
                  <span className="switch-track" />
                  <span className="switch-thumb" />
                </label>
                <span className="text-sm text-surface-300">Enable this platform</span>
              </div>

              <div className="modal-footer">
                <button
                  type="button"
                  onClick={() => { setShowModal(false); resetForm(); }}
                  className="btn-secondary"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="btn-primary"
                  disabled={isTesting || !formData.name || !formData.url || !formData.stream_key}
                >
                  {testing ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin mr-2" />
                      Testing...
                    </>
                  ) : editingPlatform ? (
                    'Save Changes'
                  ) : (
                    'Add Platform'
                  )}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}