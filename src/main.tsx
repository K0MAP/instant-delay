import React from 'react';
import ReactDOM from 'react-dom/client';
import { Toaster } from 'react-hot-toast';
import App from './App';
import './index.css';

// Initialize Tauri
import { invoke } from '@tauri-apps/api/core';

// Global shortcut types for Tauri plugin
interface GlobalShortcut {
  register(accelerator: string, handler: () => void): Promise<void>;
  unregister(accelerator: string): Promise<void>;
  unregisterAll(): Promise<void>;
}

// Check if running in Tauri
declare global {
  interface Window {
    __TAURI__?: {
      globalShortcut: GlobalShortcut;
    };
  }
}

if (window.__TAURI__) {
  invoke('get_settings').then((settings: any) => {
    // Apply theme from settings
    if (settings?.overlay_theme) {
      document.documentElement.style.setProperty('--primary-color', settings.overlay_theme.accent_color);
      document.documentElement.style.setProperty('--background-color', settings.overlay_theme.background_color);
      document.documentElement.style.setProperty('--text-color', settings.overlay_theme.text_color);
    }
  }).catch(console.error);
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
    <Toaster
      position="bottom-right"
      toastOptions={{
        duration: 4000,
        style: {
          background: '#18181b',
          color: '#fafafa',
          border: '1px solid #27272a',
          borderRadius: '12px',
          padding: '16px',
        },
        success: {
          iconTheme: {
            primary: '#22c55e',
            secondary: '#fafafa',
          },
        },
        error: {
          iconTheme: {
            primary: '#ef4444',
            secondary: '#fafafa',
          },
        },
      }}
    />
  </React.StrictMode>
);