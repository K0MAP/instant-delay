import { useLocation } from 'react-router-dom';
import { useEffect, useRef, useState } from 'react';
import { Menu, X, Bell, HelpCircle, Sun, Moon, Github, ExternalLink } from 'lucide-react';
import { useStore, useNotifications, useServerRunning } from '../store/useStore';
import { useTheme } from '../hooks/useTheme';
import clsx from 'clsx';

const pageTitles: Record<string, { title: string; description: string }> = {
  '/dashboard': { title: 'Dashboard', description: 'Monitor and control your stream delay' },
  '/platforms': { title: 'Platforms', description: 'Manage streaming destinations' },
  '/overlay': { title: 'Overlay', description: 'Configure LIVE/DELAYED overlay' },
  '/settings': { title: 'Settings', description: 'Configure application settings' },
  '/about': { title: 'About', description: 'Learn more about InstantDelay' },
};

export function Header() {
  const location = useLocation();
  const { sidebarOpen, toggleSidebar, removeNotification, clearNotifications } = useStore();
  const notifications = useNotifications();
  const serverRunning = useServerRunning();
  const { theme, toggleTheme } = useTheme();
  const unreadCount = notifications.filter((n) => !n.read).length;

  const pageInfo = pageTitles[location.pathname] || { title: 'InstantDelay', description: '' };

  const [showNotifications, setShowNotifications] = useState(false);
  const [showUserMenu, setShowUserMenu] = useState(false);
  const notificationRef = useRef<HTMLDivElement>(null);
  const userMenuRef = useRef<HTMLDivElement>(null);

  // Close dropdowns on outside click
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (notificationRef.current && !notificationRef.current.contains(event.target as Node)) {
        setShowNotifications(false);
      }
      if (userMenuRef.current && !userMenuRef.current.contains(event.target as Node)) {
        setShowUserMenu(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <header className="h-16 bg-surface-900/80 backdrop-blur-sm border-b border-surface-800 flex items-center justify-between px-4 lg:px-6 z-50 relative">
      {/* Left side - Mobile menu toggle & Page title */}
      <div className="flex items-center gap-4">
        <button
          onClick={toggleSidebar}
          className="lg:hidden p-2 rounded-lg text-surface-400 hover:text-surface-100 hover:bg-surface-800 transition-colors"
          aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
        >
          {sidebarOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
        </button>

        <div className="hidden sm:block">
          <h1 className="text-lg font-semibold text-surface-50">{pageInfo.title}</h1>
          <p className="text-xs text-surface-500">{pageInfo.description}</p>
        </div>
      </div>

      {/* Right side - Status, Notifications, Theme, User menu */}
      <div className="flex items-center gap-3">
        {/* Server Status Indicator */}
        <div className="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-800/50 border border-surface-700">
          <div className={clsx(
            'w-2 h-2 rounded-full',
            serverRunning ? 'bg-green-500' : 'bg-surface-500'
          )} aria-hidden="true" />
          <span className="text-sm font-medium text-surface-300">
            {serverRunning ? 'Server Running' : 'Server Stopped'}
          </span>
        </div>

        {/* Notifications */}
        <div className="relative" ref={notificationRef}>
          <button
            onClick={() => setShowNotifications(!showNotifications)}
            className={clsx(
              'relative p-2 rounded-lg text-surface-400 hover:text-surface-100 hover:bg-surface-800 transition-colors',
              unreadCount > 0 && 'text-primary-500'
            )}
            aria-label={`Notifications${unreadCount > 0 ? `, ${unreadCount} unread` : ''}`}
            aria-expanded={showNotifications}
          >
            <Bell className="w-5 h-5" aria-hidden="true" />
            {unreadCount > 0 && (
              <span className="absolute -top-1 -right-1 w-5 h-5 bg-red-500 text-white text-xs font-medium rounded-full flex items-center justify-center">
                {unreadCount > 9 ? '9+' : unreadCount}
              </span>
            )}
          </button>

          {showNotifications && (
            <div className="absolute right-0 mt-2 w-80 md:w-96 dropdown-menu shadow-xl border border-surface-700 animate-in">
              <div className="flex items-center justify-between p-3 border-b border-surface-800">
                <h3 className="font-semibold text-surface-50">Notifications</h3>
                <div className="flex items-center gap-2">
                  {notifications.length > 0 && (
                    <button
                      onClick={clearNotifications}
                      className="text-xs text-surface-400 hover:text-surface-200"
                    >
                      Clear all
                    </button>
                  )}
                  <button
                    onClick={() => setShowNotifications(false)}
                    className="p-1 text-surface-400 hover:text-surface-200"
                    aria-label="Close notifications"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>

              <div className="max-h-96 overflow-y-auto">
                {notifications.length === 0 ? (
                  <div className="p-6 text-center text-surface-500">
                    <Bell className="w-8 h-8 mx-auto mb-2 text-surface-600" />
                    <p className="text-sm">No notifications</p>
                  </div>
                ) : (
                  <ul className="divide-y divide-surface-800">
                    {notifications.slice(0, 10).map((notification: typeof notifications[0]) => (
                      <li key={notification.id} className="p-3 hover:bg-surface-800/50">
                        <div className="flex items-start gap-3">
                          <div className={clsx(
                            'w-2 h-2 rounded-full mt-1.5 flex-shrink-0',
                            notification.type === 'success' && 'bg-green-500',
                            notification.type === 'error' && 'bg-red-500',
                            notification.type === 'warning' && 'bg-yellow-500',
                            notification.type === 'info' && 'bg-blue-500'
                          )} />
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium text-surface-50">{notification.title}</p>
                            {notification.message && (
                              <p className="text-xs text-surface-400 mt-0.5">{notification.message}</p>
                            )}
                            <p className="text-xs text-surface-500 mt-1">
                              {new Date(notification.timestamp).toLocaleTimeString()}
                            </p>
                          </div>
                          <button
                            onClick={() => removeNotification(notification.id)}
                            className="p-1 text-surface-500 hover:text-surface-200"
                            aria-label="Dismiss"
                          >
                            <X className="w-3 h-3" />
                          </button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Theme Toggle */}
        <button
          onClick={toggleTheme}
          className="p-2 rounded-lg text-surface-400 hover:text-surface-100 hover:bg-surface-800 transition-colors"
          aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
        >
          {theme === 'dark' ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
        </button>

        {/* User Menu */}
        <div className="relative" ref={userMenuRef}>
          <button
            onClick={() => setShowUserMenu(!showUserMenu)}
            className="flex items-center gap-2 p-1.5 rounded-lg hover:bg-surface-800 transition-colors"
            aria-label="User menu"
            aria-expanded={showUserMenu}
          >
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-primary-500 to-primary-600 flex items-center justify-center">
              <span className="text-white font-medium text-sm">ID</span>
            </div>
            <span className="hidden md:block text-sm font-medium text-surface-300">Streamer</span>
          </button>

          {showUserMenu && (
            <div className="absolute right-0 mt-2 w-48 dropdown-menu shadow-xl border border-surface-700 animate-in">
              <div className="p-3 border-b border-surface-800">
                <p className="font-medium text-surface-50">InstantDelay</p>
                <p className="text-xs text-surface-500">v1.0.0</p>
              </div>
              <ul className="py-1">
                <li>
                  <a
                    href="https://github.com/yourname/instant-delay"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="dropdown-item"
                  >
                    <Github className="w-4 h-4" />
                    GitHub Repository
                  </a>
                </li>
                <li>
                  <a
                    href="https://instant-delay.com/docs"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="dropdown-item"
                  >
                    <HelpCircle className="w-4 h-4" />
                    Documentation
                  </a>
                </li>
                <li>
                  <a
                    href="https://instant-delay.com"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="dropdown-item"
                  >
                    <ExternalLink className="w-4 h-4" />
                    Website
                  </a>
                </li>
                <li><hr className="dropdown-divider" /></li>
                <li>
                  <button className="dropdown-item dropdown-item-danger w-full text-left">
                    Quit Application
                  </button>
                </li>
              </ul>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}