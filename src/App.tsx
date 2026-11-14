import { useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useNotify } from './store/useStore';
import { useServer, useDelayControl, useSettings, useSystem, useHotkeys, useOverlay, usePlatforms } from './hooks/useTauri';
import { Sidebar } from './components/Sidebar';
import { Header } from './components/Header';
import { Dashboard } from './pages/Dashboard';
import { Platforms } from './pages/Platforms';
import { Settings } from './pages/Settings';
import { Overlay } from './pages/Overlay';
import { About } from './pages/About';
import './App.css';

function App() {
  const { getSettings } = useSettings();
  const { getSystemInfo } = useSystem();
  const { registerHotkeys } = useHotkeys();
  const { getOverlayUrl } = useOverlay();
  const notify = useNotify();
  const { getStatus } = useServer();
  const { getPlatforms } = usePlatforms();
  const { skipToLive } = useDelayControl();

  // Refresh even while stopped so tray actions and backend startup are reflected.
  useEffect(() => {
    const interval = setInterval(async () => {
      await Promise.allSettled([
        getStatus(),
        getPlatforms(),
        getSystemInfo(),
      ]);
    }, 1000);

    return () => clearInterval(interval);
  }, [getStatus, getPlatforms, getSystemInfo]);

  // Initialize on mount
  useEffect(() => {
    const init = async () => {
      try {
        const [settings] = await Promise.all([
          getSettings(),
          getSystemInfo(),
          getOverlayUrl(),
          getPlatforms(),
          getStatus(),
        ]);

        // Register hotkeys if enabled
        if (settings.global_hotkeys_enabled) {
          await registerHotkeys();
        }

        // Backend setup owns auto-start; invoking it again here races startup
        // and React StrictMode can execute this effect twice in development.
      } catch (error) {
        console.error('Initialization failed:', error);
        notify('error', 'Failed to initialize', 'Check console for details');
      }
    };

    init();
  }, [getSettings, getSystemInfo, getOverlayUrl, getPlatforms, getStatus, registerHotkeys, notify]);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl/Cmd + Shift + D - Toggle delay
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'D') {
        e.preventDefault();
        import('@tauri-apps/api/core').then(({ invoke }) => invoke('toggle_delay'));
      }
      // Ctrl/Cmd + Shift + Up - Increase delay
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowUp') {
        e.preventDefault();
        import('@tauri-apps/api/core').then(({ invoke }) => invoke('increase_delay'));
      }
      // Ctrl/Cmd + Shift + Down - Decrease delay
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowDown') {
        e.preventDefault();
        import('@tauri-apps/api/core').then(({ invoke }) => invoke('decrease_delay'));
      }
      // Ctrl/Cmd + Shift + L - Skip to live
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'L') {
        e.preventDefault();
        skipToLive();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [skipToLive]);

  return (
    <BrowserRouter>
      <div className="app min-h-screen bg-surface-950 text-surface-50 flex">
        <Sidebar />
        <div className="flex-1 flex flex-col min-w-0">
          <Header />
          <main className="flex-1 p-6 overflow-auto">
            <Routes>
              <Route path="/" element={<Navigate to="/dashboard" replace />} />
              <Route path="/dashboard" element={<Dashboard />} />
              <Route path="/platforms" element={<Platforms />} />
              <Route path="/overlay" element={<Overlay />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="/about" element={<About />} />
              <Route path="*" element={<Navigate to="/dashboard" replace />} />
            </Routes>
          </main>
        </div>
      </div>
    </BrowserRouter>
  );
}

export default App;