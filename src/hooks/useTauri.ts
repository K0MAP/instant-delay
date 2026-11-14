import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useStore } from '../store/useStore';
import type {
  Settings,
  PlatformOutput,
  DelayMode,
  SystemInfo,
  ServerStatusResponse,
  OverlayData,
  StreamDeckEvent,
  HotkeyConfig
} from '../types';

// Generic invoke wrapper with error handling
async function invokeCommand<T>(command: string, args?: any): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    console.error(`Tauri command ${command} failed:`, error);
    throw error;
  }
}

// Server commands
export function useServer() {
  const { setServerRunning, setServerStatus } = useStore();

  const startServer = useCallback(async () => {
    await invokeCommand('start_rtmp_server');
    setServerRunning(true);
  }, [setServerRunning]);

  const stopServer = useCallback(async () => {
    await invokeCommand('stop_rtmp_server');
    setServerRunning(false);
  }, [setServerRunning]);

  const getStatus = useCallback(async () => {
    const status = await invokeCommand<ServerStatusResponse>('get_rtmp_status');
    setServerStatus(status);
    return status;
  }, [setServerStatus]);

  return { startServer, stopServer, getStatus };
}

// Delay control commands
export function useDelayControl() {
  const { setDelaySec, setDelayMode, setBuffering, delaySec, delayMode } = useStore();

  const setDelay = useCallback(async (seconds: number) => {
    await invokeCommand('set_delay', { seconds });
    setDelaySec(seconds);
  }, [setDelaySec]);

  const toggleDelay = useCallback(async () => {
    const mode = await invokeCommand<DelayMode>('toggle_delay');
    setDelayMode(mode);
    return mode;
  }, [setDelayMode]);

  const increaseDelay = useCallback(async () => {
    const newDelay = await invokeCommand<number>('increase_delay');
    setDelaySec(newDelay);
    return newDelay;
  }, [setDelaySec]);

  const decreaseDelay = useCallback(async () => {
    const newDelay = await invokeCommand<number>('decrease_delay');
    setDelaySec(newDelay);
    return newDelay;
  }, [setDelaySec]);

  const skipToLive = useCallback(async () => {
    await invokeCommand('skip_to_live');
    setBuffering(false, 100);
  }, [setBuffering]);

  const setMode = useCallback(async (mode: DelayMode) => {
    await invokeCommand('set_mode', { mode });
    setDelayMode(mode);
  }, [setDelayMode]);

  return {
    delaySec,
    delayMode,
    setDelay,
    toggleDelay,
    increaseDelay,
    decreaseDelay,
    skipToLive,
    setMode,
  };
}

// Platform commands
export function usePlatforms() {
  const { platforms, setPlatforms, removePlatform, updatePlatform } = useStore();

  const getPlatforms = useCallback(async () => {
    const platforms = await invokeCommand<PlatformOutput[]>('get_platform_outputs');
    setPlatforms(platforms);
    return platforms;
  }, [setPlatforms]);

  const addPlatformOutput = useCallback(async (platform: PlatformOutput) => {
    const id = await invokeCommand<string>('add_platform_output', { output: platform });
    await getPlatforms();
    return id;
  }, [getPlatforms]);

  const removePlatformOutput = useCallback(async (id: string) => {
    await invokeCommand('remove_platform_output', { id });
    removePlatform(id);
  }, [removePlatform]);

  const updatePlatformOutput = useCallback(async (platform: PlatformOutput) => {
    await invokeCommand('update_platform_output', { output: platform });
    updatePlatform(platform);
  }, [updatePlatform]);

  const testConnection = useCallback(async (platform: PlatformOutput) => {
    return await invokeCommand<boolean>('test_platform_connection', { output: platform });
  }, []);

  return {
    platforms,
    getPlatforms,
    addPlatformOutput,
    removePlatformOutput,
    updatePlatformOutput,
    testConnection,
  };
}

// Settings commands
export function useSettings() {
  const { settings, setSettings, updateSetting } = useStore();

  const getSettings = useCallback(async () => {
    const settings = await invokeCommand<Settings>('get_settings');
    setSettings(settings);
    return settings;
  }, [setSettings]);

  const updateSettings = useCallback(async (newSettings: Settings) => {
    await invokeCommand('update_settings', { newSettings });
    setSettings(newSettings);
  }, [setSettings]);

  const resetSettings = useCallback(async () => {
    const settings = await invokeCommand<Settings>('reset_settings');
    setSettings(settings);
    return settings;
  }, [setSettings]);

  const updateSettingValue = useCallback(<K extends keyof Settings>(key: K, value: Settings[K]) => {
    updateSetting(key, value);
  }, [updateSetting]);

  return {
    settings,
    getSettings,
    updateSettings,
    resetSettings,
    updateSetting: updateSettingValue,
  };
}

// Overlay commands
export function useOverlay() {
  const { setOverlayData, overlayData } = useStore();

  const getOverlayUrl = useCallback(async () => {
    return await invokeCommand<string>('get_overlay_url');
  }, []);

  const regenerateOverlay = useCallback(async () => {
    await invokeCommand('regenerate_overlay');
  }, []);

  // Subscribe to overlay data updates
  useEffect(() => {
    let unlisten: UnlistenFn;

    const setupListener = async () => {
      unlisten = await listen<OverlayData>('overlay_data', (event) => {
        setOverlayData(event.payload);
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, [setOverlayData]);

  return {
    overlayData,
    getOverlayUrl,
    regenerateOverlay,
  };
}

// Hotkey commands
export function useHotkeys() {
  const { settings } = useStore();
  const registeredShortcuts = useRef<Set<string>>(new Set());

  const acceleratorToHandler = (accelerator: string): string => {
    return `hotkey_${accelerator.replace(/\+/g, '_').toLowerCase()}`;
  };

  const parseHotkey = (hotkeyStr: string): string => {
    const parts = hotkeyStr.split('+');
    const modifiers: string[] = [];
    let mainKey = '';

    for (const part of parts) {
      const lower = part.trim().toLowerCase();
      switch (lower) {
        case 'control':
        case 'ctrl':
          modifiers.push('Control');
          break;
        case 'shift':
          modifiers.push('Shift');
          break;
        case 'alt':
        case 'option':
          modifiers.push('Alt');
          break;
        case 'meta':
        case 'command':
        case 'cmd':
        case 'super':
          modifiers.push('Meta');
          break;
        default:
          mainKey = keyToAccelerator(lower);
      }
    }

    let accelerator = modifiers.join('+');
    if (mainKey) {
      if (accelerator) accelerator += '+';
      accelerator += mainKey;
    }
    return accelerator;
  };

  const keyToAccelerator = (key: string): string => {
    const keyMap: Record<string, string> = {
      up: 'Up', down: 'Down', left: 'Left', right: 'Right',
      space: 'Space', enter: 'Enter', escape: 'Escape', esc: 'Escape',
      tab: 'Tab', backspace: 'Backspace', delete: 'Delete',
      home: 'Home', end: 'End', pageup: 'PageUp', pagedown: 'PageDown',
      insert: 'Insert',
      f1: 'F1', f2: 'F2', f3: 'F3', f4: 'F4', f5: 'F5', f6: 'F6',
      f7: 'F7', f8: 'F8', f9: 'F9', f10: 'F10', f11: 'F11', f12: 'F12',
      '0': '0', '1': '1', '2': '2', '3': '3', '4': '4', '5': '5',
      '6': '6', '7': '7', '8': '8', '9': '9',
      a: 'A', b: 'B', c: 'C', d: 'D', e: 'E', f: 'F', g: 'G', h: 'H',
      i: 'I', j: 'J', k: 'K', l: 'L', m: 'M', n: 'N', o: 'O', p: 'P',
      q: 'Q', r: 'R', s: 'S', t: 'T', u: 'U', v: 'V', w: 'W', x: 'X',
      y: 'Y', z: 'Z',
    };
    return keyMap[key] || key.toUpperCase();
  };

  const registerHotkeys = useCallback(async () => {
    if (!settings?.global_hotkeys_enabled) return;
    if (!window.__TAURI__?.globalShortcut) {
      console.warn('Tauri global shortcut plugin not available');
      return;
    }

    const hotkeys = settings.hotkeys;
    const shortcutConfig = [
      { key: 'toggle_delay', action: 'toggle_delay' },
      { key: 'increase_delay', action: 'increase_delay' },
      { key: 'decrease_delay', action: 'decrease_delay' },
      { key: 'skip_to_live', action: 'skip_to_live' },
      { key: 'toggle_video', action: 'toggle_video' },
      { key: 'toggle_audio', action: 'toggle_audio' },
    ] as const;

    for (const { key, action } of shortcutConfig) {
      const hotkeyStr = hotkeys[key as keyof typeof hotkeys];
      if (!hotkeyStr) continue;

      try {
        const accelerator = parseHotkey(hotkeyStr);
        const handlerName = acceleratorToHandler(accelerator);

        // Register with Tauri global shortcut plugin
        await window.__TAURI__.globalShortcut.register(accelerator, async () => {
          try {
            await invoke('execute_hotkey', { handler: handlerName });
          } catch (error) {
            console.error(`Hotkey ${accelerator} failed:`, error);
          }
        });

        registeredShortcuts.current.add(accelerator);
        console.log(`Registered global shortcut: ${accelerator} -> ${action}`);
      } catch (error) {
        console.error(`Failed to register hotkey ${key}:`, error);
      }
    }
  }, [settings]);

  const unregisterHotkeys = useCallback(async () => {
    if (!window.__TAURI__?.globalShortcut) {
      console.warn('Tauri global shortcut plugin not available');
      return;
    }
    try {
      await window.__TAURI__.globalShortcut.unregisterAll();
      registeredShortcuts.current.clear();
      console.log('Unregistered all global shortcuts');
    } catch (error) {
      console.error('Failed to unregister hotkeys:', error);
    }
  }, []);

  const getHotkeyConfig = useCallback(async () => {
    return await invokeCommand<HotkeyConfig>('get_hotkey_config');
  }, []);

  const updateHotkeyConfig = useCallback(async (config: HotkeyConfig) => {
    await invokeCommand('update_hotkey_config', { config });
    // Re-register with new config
    await unregisterHotkeys();
    // Trigger re-registration by updating settings
    // The registerHotkeys will be called again by the effect in App.tsx
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      unregisterHotkeys();
    };
  }, [unregisterHotkeys]);

  return {
    hotkeys: settings?.hotkeys ?? {},
    registerHotkeys,
    unregisterHotkeys,
    getHotkeyConfig,
    updateHotkeyConfig,
  };
}

// System commands
export function useSystem() {
  const { setSystemInfo, systemInfo } = useStore();

  const getSystemInfo = useCallback(async () => {
    const info = await invokeCommand<SystemInfo>('get_system_info');
    setSystemInfo(info);
    return info;
  }, [setSystemInfo]);

  const checkObsRunning = useCallback(async () => {
    return await invokeCommand<boolean>('check_obs_running');
  }, []);

  return {
    systemInfo,
    getSystemInfo,
    checkObsRunning,
  };
}

// Stream Deck commands
export function useStreamDeck() {
  const getStatus = useCallback(async () => {
    return await invokeCommand<boolean>('get_streamdeck_status');
  }, []);

  const sendUpdate = useCallback(async (event: StreamDeckEvent) => {
    await invokeCommand('send_streamdeck_update', { event });
  }, []);

  // Listen for Stream Deck events
  useEffect(() => {
    let unlisten: UnlistenFn;

    const setupListener = async () => {
      unlisten = await listen<StreamDeckEvent>('streamdeck_event', (event) => {
        console.log('Stream Deck event:', event.payload);
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return {
    getStatus,
    sendUpdate,
  };
}

// Notifications
export function useNotifications() {
  const { addNotification, removeNotification, clearNotifications, notifications } = useStore();

  const notify = useCallback((
    type: 'info' | 'success' | 'warning' | 'error',
    title: string,
    message?: string,
    duration?: number
  ) => {
    addNotification({ type, title, message, duration });
  }, [addNotification]);

  const dismiss = useCallback((id: string) => {
    removeNotification(id);
  }, [removeNotification]);

  const clear = useCallback(() => {
    clearNotifications();
  }, [clearNotifications]);

  return {
    notifications,
    notify,
    dismiss,
    clear,
    info: (title: string, message?: string) => notify('info', title, message),
    success: (title: string, message?: string) => notify('success', title, message),
    warning: (title: string, message?: string) => notify('warning', title, message),
    error: (title: string, message?: string) => notify('error', title, message),
  };
}

// Auto-refresh hook
export function useAutoRefresh(intervalMs: number = 1000) {
  const { getStatus } = useServer();
  const { getPlatforms } = usePlatforms();
  const { getSystemInfo } = useSystem();

  useEffect(() => {
    const refresh = async () => {
      await Promise.allSettled([
        getStatus(),
        getPlatforms(),
        getSystemInfo(),
      ]);
    };

    // Initial load
    refresh();

    // Set up interval
    const id = setInterval(refresh, intervalMs);

    return () => clearInterval(id);
  }, [intervalMs, getStatus, getPlatforms, getSystemInfo]);
}

// Debounced callback hook
export function useDebouncedCallback<T extends (...args: any[]) => any>(
  callback: T,
  delay: number
): T {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(callback);

  callbackRef.current = callback;

  return useCallback(
    ((...args: any[]) => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      timeoutRef.current = setTimeout(() => {
        callbackRef.current(...args);
      }, delay);
    }) as T,
    [delay]
  );
}

// Local storage sync hook
export function useLocalStorage<T>(key: string, initialValue: T) {
  const [storedValue, setStoredValue] = useState<T>(() => {
    try {
      const item = window.localStorage.getItem(key);
      return item ? JSON.parse(item) : initialValue;
    } catch (error) {
      console.error(`Error reading localStorage key "${key}":`, error);
      return initialValue;
    }
  });

  const setValue = useCallback((value: T | ((val: T) => T)) => {
    try {
      const valueToStore = value instanceof Function ? value(storedValue) : value;
      setStoredValue(valueToStore);
      window.localStorage.setItem(key, JSON.stringify(valueToStore));
    } catch (error) {
      console.error(`Error setting localStorage key "${key}":`, error);
    }
  }, [key, storedValue]);

  return [storedValue, setValue] as const;
}