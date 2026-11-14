import { NavLink, useLocation } from 'react-router-dom';
import {
  LayoutDashboard,
  Globe,
  Monitor,
  Settings,
  Info,
  ChevronLeft,
  ChevronRight,
  Zap,
  Wifi,
  Activity
} from 'lucide-react';
import { useStore, useSidebarOpen, useServerRunning, useDelaySec, useDelayMode, useIsBuffering } from '../store/useStore';
import clsx from 'clsx';

const navigation = [
  { path: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { path: '/platforms', label: 'Platforms', icon: Globe },
  { path: '/overlay', label: 'Overlay', icon: Monitor },
  { path: '/settings', label: 'Settings', icon: Settings },
  { path: '/about', label: 'About', icon: Info },
];

export function Sidebar() {
  const location = useLocation();
  const sidebarOpen = useSidebarOpen();
  const { toggleSidebar } = useStore();
  const serverRunning = useServerRunning();
  const delaySec = useDelaySec();
  const delayMode = useDelayMode();
  const isBuffering = useIsBuffering();

  const isMobile = window.innerWidth < 1024;

  return (
    <>
      {/* Mobile overlay */}
      {isMobile && sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-30 lg:hidden"
          onClick={() => toggleSidebar()}
          aria-hidden="true"
        />
      )}

      {/* Sidebar */}
      <aside
        className={clsx(
          'sidebar flex flex-col bg-surface-900/80 backdrop-blur-sm border-r border-surface-800 transition-transform duration-300 ease-in-out z-40',
          isMobile ? (sidebarOpen ? 'sidebar-open translate-x-0' : 'sidebar-closed -translate-x-full') : 'w-64',
          'lg:w-64 lg:translate-x-0'
        )}
        aria-label="Main navigation"
      >
        {/* Logo/Brand */}
        <div className="flex items-center justify-between h-16 px-4 border-b border-surface-800">
          <NavLink to="/dashboard" className="flex items-center gap-2" aria-label="InstantDelay Home">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-primary-500 to-primary-600 flex items-center justify-center">
              <Zap className="w-5 h-5 text-white" aria-hidden="true" />
            </div>
            <span className="font-bold text-lg text-surface-50">InstantDelay</span>
          </NavLink>

          {isMobile && (
            <button
              onClick={toggleSidebar}
              className="p-1.5 rounded-lg text-surface-400 hover:text-surface-100 hover:bg-surface-800 transition-colors"
              aria-label="Close sidebar"
            >
              <ChevronRight className="w-5 h-5" aria-hidden="true" />
            </button>
          )}
        </div>

        {/* Server Status */}
        <div className="p-4 border-b border-surface-800">
          <div className="flex items-center gap-3">
            <div className={clsx(
              'w-3 h-3 rounded-full flex-shrink-0',
              serverRunning
                ? (isBuffering ? 'status-dot-buffering' : 'status-dot-live')
                : 'status-dot-offline'
            )}
            aria-hidden="true" />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-surface-300 truncate">
                {serverRunning ? (isBuffering ? 'Buffering...' : 'Live') : 'Offline'}
              </p>
              <p className="text-xs text-surface-500 truncate">
                {serverRunning ? `${delaySec}s ${delayMode}` : 'Click to start'}
              </p>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav className="flex-1 p-3 space-y-1" aria-label="Main navigation">
          {navigation.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
              key={item.path}
              to={item.path}
              className={({ isActive }) => clsx(
                'sidebar-item',
                isActive && 'sidebar-item-active'
              )}
              aria-current={location.pathname === item.path ? 'page' : undefined}
              onClick={() => isMobile && toggleSidebar()}
            >
              <Icon className="w-5 h-5 flex-shrink-0" aria-hidden="true" />
              <span>{item.label}</span>
            </NavLink>
            );
          })}
        </nav>

        {/* Quick Actions */}
        <div className="p-3 border-t border-surface-800">
          <p className="px-3 py-1 text-xs font-semibold text-surface-500 uppercase tracking-wider">
            Quick Actions
          </p>
          <div className="space-y-2">
            <button
              className="w-full sidebar-item justify-center gap-2"
              onClick={() => import('@tauri-apps/api/core').then(({ invoke }) => invoke('toggle_delay'))}
              disabled={!serverRunning}
              aria-label={serverRunning ? 'Toggle delay' : 'Start server first'}
            >
              <Activity className="w-5 h-5" aria-hidden="true" />
              <span>Toggle Delay</span>
            </button>
            <button
              className="w-full sidebar-item justify-center gap-2"
              onClick={() => import('@tauri-apps/api/core').then(({ invoke }) => invoke('skip_to_live'))}
              disabled={!serverRunning || !isBuffering}
              aria-label={isBuffering ? 'Skip to live' : 'Not buffering'}
            >
              <Wifi className="w-5 h-5" aria-hidden="true" />
              <span>Skip to Live</span>
            </button>
          </div>
        </div>

        {/* Footer */}
        <div className="p-3 border-t border-surface-800">
          <div className="flex items-center justify-between text-xs text-surface-500">
            <span>v1.0.0</span>
            <button
              onClick={toggleSidebar}
              className="p-1.5 rounded-lg hover:bg-surface-800 transition-colors lg:hidden"
              aria-label="Collapse sidebar"
            >
              <ChevronLeft className="w-4 h-4" aria-hidden="true" />
            </button>
          </div>
        </div>
      </aside>

      {/* Desktop toggle button */}
      {!isMobile && (
        <button
          onClick={toggleSidebar}
          className="fixed left-64 top-4 z-20 p-1.5 rounded-lg bg-surface-900 border border-surface-700 text-surface-400 hover:text-surface-100 hover:bg-surface-800 transition-colors lg:hidden"
          aria-label="Toggle sidebar"
          style={{ transform: sidebarOpen ? 'translateX(0)' : 'translateX(-100%)' }}
        >
          <ChevronLeft className="w-5 h-5" aria-hidden="true" />
        </button>
      )}
    </>
  );
}