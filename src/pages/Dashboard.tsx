import {
  Zap,
  Clock,
  Server,
  Activity,
  RotateCcw,
  Plus,
  Minus,
  SkipForward,
  Power,
  ExternalLink,
  Loader2,
  X
} from 'lucide-react';
import { useIsBuffering, useBufferFillPercent, useServerRunning } from '../store/useStore';
import { useServer, useDelayControl, useSystem, useNotifications } from '../hooks/useTauri';
import { useState, useEffect } from 'react';
import clsx from 'clsx';
import { useTranslation } from '../i18n';

export function Dashboard() {
  const { t } = useTranslation();
  const { startServer, stopServer } = useServer();
  const {
    delaySec,
    delayMode,
    setDelay,
    toggleDelay,
    increaseDelay,
    decreaseDelay,
    skipToLive
  } = useDelayControl();
  const { systemInfo } = useSystem();
  const { notify } = useNotifications();
  const serverRunning = useServerRunning();
  const isBuffering = useIsBuffering();
  const bufferFillPercent = useBufferFillPercent();

  const [localDelay, setLocalDelay] = useState(delaySec);
  const [changingDelay, setChangingDelay] = useState(false);

  // Sync local delay with store
  useEffect(() => {
    setLocalDelay(delaySec);
  }, [delaySec]);

  const handleStartServer = async () => {
    try {
      await startServer();
      notify('success', 'Server started', 'RTMP server is now running');
    } catch (error) {
      notify('error', 'Failed to start server', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleStopServer = async () => {
    try {
      await stopServer();
      notify('info', 'Server stopped', 'RTMP server has been stopped');
    } catch (error) {
      notify('error', 'Failed to stop server', error instanceof Error ? error.message : 'Unknown error');
    }
  };

  const handleDelayChange = async (newDelay: number) => {
    if (newDelay < 1 || newDelay > 600) return;
    setChangingDelay(true);
    try {
      await setDelay(newDelay);
      setLocalDelay(newDelay);
    } catch (error) {
      notify('error', 'Failed to set delay', error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setChangingDelay(false);
    }
  };

  const handleQuickDelay = async (delta: number) => {
    const newDelay = Math.max(1, Math.min(600, delaySec + delta));
    await handleDelayChange(newDelay);
  };

  return (
    <div className="space-y-6 animate-in">
      {/* Header with Server Control */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-surface-50">{t('Dashboard')}</h2>
          <p className="text-surface-400 text-sm mt-1">{t('Monitor and control your stream delay in real-time')}</p>
        </div>

        <div className="flex items-center gap-3">
          {!serverRunning ? (
            <button
              onClick={handleStartServer}
              className="btn-primary flex items-center gap-2"
              disabled={changingDelay}
            >
              <Power className="w-4 h-4" />
              {t('Start Server')}
            </button>
          ) : (
            <>
              <button
                onClick={handleStopServer}
                className="btn-secondary flex items-center gap-2"
                disabled={changingDelay}
              >
                <Power className="w-4 h-4" />
                {t('Stop Server')}
              </button>
              <button
                onClick={skipToLive}
                className="btn-ghost flex items-center gap-2"
                disabled={!isBuffering || changingDelay}
              >
                <SkipForward className="w-4 h-4" />
                {t('Skip to Live')}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Main Stats Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Delay Status Card */}
        <div className="stat-card">
          <div className="flex items-start justify-between">
            <div>
              <p className="stat-label">{t('Current Delay')}</p>
              <div className="delay-display">
                <span className="delay-value text-primary-400">{delaySec}</span>
                <span className="delay-unit">{t('seconds')}</span>
              </div>
            </div>
            <div className={clsx(
              'w-12 h-12 rounded-xl flex items-center justify-center',
              isBuffering ? 'bg-orange-500/20' : 'bg-green-500/20'
            )}>
              {isBuffering ? (
                <Loader2 className="w-6 h-6 text-orange-500 animate-spin" />
              ) : (
                <Activity className="w-6 h-6 text-green-500" />
              )}
            </div>
          </div>

          {/* Buffer Progress */}
          <div className="mt-4">
            <div className="flex items-center justify-between text-xs mb-1">
              <span className="text-surface-400">{t('Buffer Fill')}</span>
              <span className="font-mono text-surface-300">{bufferFillPercent.toFixed(1)}%</span>
            </div>
            <div className="buffer-container">
              <div
                className={clsx('buffer-fill', isBuffering && 'buffering')}
                style={{ width: `${bufferFillPercent}%` }}
              />
            </div>
            <p className="text-xs text-surface-500 mt-1">
              {isBuffering ? t('Buffering...') : t('Ready for delayed output')}
            </p>
          </div>
        </div>

        {/* Mode Card */}
        <div className="stat-card">
          <div className="flex items-start justify-between">
            <div>
              <p className="stat-label">{t('Delay Mode')}</p>
              <p className="text-2xl font-bold text-surface-50 capitalize">{t(delayMode.charAt(0).toUpperCase() + delayMode.slice(1))}</p>
            </div>
            <div className={clsx(
              'w-12 h-12 rounded-xl flex items-center justify-center',
              delayMode === 'instant' ? 'bg-primary-500/20' :
              delayMode === 'normal' ? 'bg-blue-500/20' : 'bg-surface-700/50'
            )}>
              {delayMode === 'instant' && <Zap className="w-6 h-6 text-primary-500" />}
              {delayMode === 'normal' && <Clock className="w-6 h-6 text-blue-500" />}
              {delayMode === 'disabled' && <X className="w-6 h-6 text-surface-500" />}
            </div>
          </div>

          <div className="mt-4 flex gap-2">
            <button
              onClick={() => toggleDelay()}
              className={clsx(
                'btn flex-1',
                delayMode === 'instant' ? 'btn-primary' : 'btn-secondary'
              )}
              disabled={changingDelay}
            >
              {t('Instant')}
            </button>
            <button
              onClick={() => import('@tauri-apps/api/core').then(({ invoke }) => invoke('set_mode', { mode: 'normal' }))}
              className={clsx(
                'btn flex-1',
                delayMode === 'normal' ? 'btn-primary' : 'btn-secondary'
              )}
              disabled={changingDelay}
            >
              {t('Normal')}
            </button>
            <button
              onClick={() => import('@tauri-apps/api/core').then(({ invoke }) => invoke('set_mode', { mode: 'disabled' }))}
              className={clsx(
                'btn flex-1',
                delayMode === 'disabled' ? 'btn-danger' : 'btn-secondary'
              )}
              disabled={changingDelay}
            >
              {t('Disabled')}
            </button>
          </div>
        </div>

        {/* Server Stats Card */}
        <div className="stat-card">
          <p className="stat-label">{t('Server Stats')}</p>
          <div className="grid grid-cols-2 gap-4 mt-4">
            <div>
              <p className="stat-value font-mono" id="connections">-</p>
              <p className="stat-label">{t('Connections')}</p>
            </div>
            <div>
              <p className="stat-value font-mono" id="uptime">-</p>
              <p className="stat-label">{t('Uptime')}</p>
            </div>
            <div>
              <p className="stat-value font-mono" id="bytesIn">-</p>
              <p className="stat-label">{t('Data In')}</p>
            </div>
            <div>
              <p className="stat-value font-mono" id="bytesOut">-</p>
              <p className="stat-label">{t('Data Out')}</p>
            </div>
          </div>
        </div>

        {/* System Info Card */}
        <div className="stat-card">
          <p className="stat-label">{t('System')}</p>
          <div className="space-y-3 mt-4">
            <div className="flex justify-between text-sm">
              <span className="text-surface-400">{t('CPU')}</span>
              <span className="text-surface-300 font-mono" id="cpuUsage">-</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-surface-400">{t('Memory')}</span>
              <span className="text-surface-300 font-mono" id="memoryUsage">-</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-surface-400">OBS Input</span>
              <span className="text-surface-300 font-mono" id="obsStatus">Checking...</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-surface-400">Platform</span>
              <span className="text-surface-300 font-mono">{systemInfo?.os || 'Unknown'}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Delay Control Panel */}
      <div className="card p-6">
        <div className="flex items-center justify-between mb-6">
          <h3 className="text-lg font-semibold text-surface-50">Delay Control</h3>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                className="custom-checkbox w-4 h-4"
                checked={delayMode === 'instant'}
                onChange={() => toggleDelay()}
                disabled={changingDelay}
              />
              <span className="text-sm text-surface-300">Instant Mode</span>
            </label>
          </div>
        </div>

        {/* Large Delay Slider */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <label htmlFor="delay-slider" className="label">Delay (seconds)</label>
            <div className="flex items-center gap-4">
              <span className="text-lg font-mono text-primary-400 min-w-[4rem] text-right">
                {localDelay}s
              </span>
              <input
                type="range"
                id="delay-slider"
                min="1"
                max="600"
                value={localDelay}
                onChange={(e) => setLocalDelay(parseInt(e.target.value))}
                onMouseUp={() => handleDelayChange(localDelay)}
                onTouchEnd={() => handleDelayChange(localDelay)}
                className="flex-1 h-2 bg-surface-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                disabled={changingDelay || !serverRunning}
              />
            </div>
          </div>

          {/* Quick delay buttons */}
          <div className="flex flex-wrap gap-2">
            {[-30, -15, -5, 5, 15, 30].map((delta) => (
              <button
                key={delta}
                onClick={() => handleQuickDelay(delta)}
                className="btn-secondary text-sm"
                disabled={changingDelay || !serverRunning || delaySec + delta < 1 || delaySec + delta > 600}
              >
                {delta > 0 ? '+' : ''}{delta}s
              </button>
            ))}
          </div>
        </div>

        {/* Quick Actions */}
        <div className="flex flex-wrap gap-3">
          <button
            onClick={increaseDelay}
            className="btn-primary flex-1 sm:flex-none"
            disabled={changingDelay || !serverRunning || delaySec >= 600}
          >
            <Plus className="w-4 h-4 mr-2" />
            Increase (+5s)
          </button>

          <button
            onClick={decreaseDelay}
            className="btn-secondary flex-1 sm:flex-none"
            disabled={changingDelay || !serverRunning || delaySec <= 1}
          >
            <Minus className="w-4 h-4 mr-2" />
            Decrease (-5s)
          </button>

          <button
            onClick={toggleDelay}
            className={clsx(
              'btn flex-1 sm:flex-none',
              delayMode === 'instant' ? 'btn-primary' : 'btn-secondary'
            )}
            disabled={changingDelay || !serverRunning}
          >
            <RotateCcw className="w-4 h-4 mr-2" />
            Toggle Delay
          </button>

          <button
            onClick={skipToLive}
            className={clsx(
              'btn flex-1 sm:flex-none',
              isBuffering ? 'btn-success' : 'btn-ghost'
            )}
            disabled={!isBuffering || changingDelay || !serverRunning}
          >
            <SkipForward className="w-4 h-4 mr-2" />
            Skip to Live
          </button>
        </div>
      </div>

      {/* Platform Status Overview */}
      <div className="card p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-surface-50">Platform Outputs</h3>
          <a href="/platforms" className="btn-ghost text-sm">
            Manage Platforms
            <ExternalLink className="w-3 h-3 ml-1" />
          </a>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="text-left text-sm text-surface-500 border-b border-surface-800">
                <th className="pb-2 font-medium">Platform</th>
                <th className="pb-2 font-medium">Status</th>
                <th className="pb-2 font-medium">Data Sent</th>
                <th className="pb-2 font-medium">Uptime</th>
                <th className="pb-2 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr className="text-center text-surface-500">
                <td className="py-8" colSpan={5}>
                  <Server className="w-12 h-12 mx-auto mb-2 text-surface-600" />
                  <p>No platform outputs configured</p>
                  <a href="/platforms" className="btn-primary mt-3 inline-flex items-center gap-2">
                    <Plus className="w-4 h-4" />
                    Add Platform
                  </a>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}