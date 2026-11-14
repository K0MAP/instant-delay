import { create } from 'zustand';
import { persist, subscribeWithSelector } from 'zustand/middleware';
import type { Settings, PlatformOutput, DelayMode, OverlayData, SystemInfo, ServerStatusResponse } from '../types';

interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message?: string;
  timestamp: number;
  duration?: number;
  read?: boolean;
}

interface AppState {
  // Server state
  serverRunning: boolean;
  serverStatus: ServerStatusResponse | null;
  delaySec: number;
  delayMode: DelayMode;
  isBuffering: boolean;
  bufferFillPercent: number;

  // Platform outputs
  platforms: PlatformOutput[];

  // Settings
  settings: Settings | null;

  // System
  systemInfo: SystemInfo | null;

  // UI state
  activeTab: string;
  sidebarOpen: boolean;
  notifications: Notification[];

  // Overlay
  overlayData: OverlayData | null;

  // Actions
  setServerRunning: (running: boolean) => void;
  setServerStatus: (status: ServerStatusResponse) => void;
  setDelaySec: (seconds: number) => void;
  setDelayMode: (mode: DelayMode) => void;
  setBuffering: (buffering: boolean, fillPercent?: number) => void;
  setPlatforms: (platforms: PlatformOutput[]) => void;
  addPlatform: (platform: PlatformOutput) => void;
  removePlatform: (id: string) => void;
  updatePlatform: (platform: PlatformOutput) => void;
  setSettings: (settings: Settings) => void;
  updateSetting: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  setSystemInfo: (info: SystemInfo) => void;
  setActiveTab: (tab: string) => void;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  addNotification: (notification: Omit<Notification, 'id' | 'timestamp'>) => void;
  removeNotification: (id: string) => void;
  clearNotifications: () => void;
  setOverlayData: (data: OverlayData) => void;
  notify: (type: Notification['type'], title: string, message?: string, duration?: number) => void;
}

export const useStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        // Initial state
        serverRunning: false,
        serverStatus: null,
        delaySec: 10,
        delayMode: 'instant',
        isBuffering: true,
        bufferFillPercent: 0,
        platforms: [],
        settings: null,
        systemInfo: null,
        activeTab: 'dashboard',
        sidebarOpen: true,
        notifications: [],
        overlayData: null,

        // Actions
        setServerRunning: (running) => set({ serverRunning: running }),

        setServerStatus: (status) => set({
          serverStatus: status,
          serverRunning: status.running,
          delaySec: status.current_delay_sec,
          delayMode: status.delay_mode,
          isBuffering: status.is_buffering,
          bufferFillPercent: status.buffer_fill_percent,
        }),

        setDelaySec: (seconds) => set({ delaySec: seconds }),

        setDelayMode: (mode) => set({ delayMode: mode }),

        setBuffering: (buffering, fillPercent) => set({
          isBuffering: buffering,
          bufferFillPercent: fillPercent ?? get().bufferFillPercent,
        }),

        setPlatforms: (platforms) => set({ platforms }),

        addPlatform: (platform) => set((state) => ({
          platforms: [...state.platforms, platform],
        })),

        removePlatform: (id) => set((state) => ({
          platforms: state.platforms.filter((p) => p.id !== id),
        })),

        updatePlatform: (platform) => set((state) => ({
          platforms: state.platforms.map((p) => p.id === platform.id ? platform : p),
        })),

        setSettings: (settings) => set({ settings }),

        updateSetting: (key, value) => set((state) => ({
          settings: state.settings ? { ...state.settings, [key]: value } : null,
        })),

        setSystemInfo: (info) => set({ systemInfo: info }),

        setActiveTab: (tab) => set({ activeTab: tab }),

        toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),

        setSidebarOpen: (open) => set({ sidebarOpen: open }),

        addNotification: (notification) => set((state) => ({
          notifications: [
            {
              ...notification,
              id: crypto.randomUUID(),
              timestamp: Date.now(),
              read: false,
            },
            ...state.notifications.slice(0, 49), // Keep max 50
          ],
        })),

        removeNotification: (id) => set((state) => ({
          notifications: state.notifications.filter((n) => n.id !== id),
        })),

        clearNotifications: () => set({ notifications: [] }),

        // Convenience method for adding notifications
        notify: (type: Notification['type'], title: string, message?: string, duration?: number) => {
          get().addNotification({ type, title, message, duration });
        },

        setOverlayData: (data) => set({ overlayData: data }),
      }),
      {
        name: 'instant-delay-store',
        partialize: (state) => ({
          activeTab: state.activeTab,
          sidebarOpen: state.sidebarOpen,
          // Don't persist runtime state
        }),
      }
    )
  )
);

// Selectors for performance
export const useServerRunning = () => useStore((state) => state.serverRunning);
export const useDelaySec = () => useStore((state) => state.delaySec);
export const useDelayMode = () => useStore((state) => state.delayMode);
export const useIsBuffering = () => useStore((state) => state.isBuffering);
export const useBufferFillPercent = () => useStore((state) => state.bufferFillPercent);
export const usePlatforms = () => useStore((state) => state.platforms);
export const useSettings = () => useStore((state) => state.settings);
export const useActiveTab = () => useStore((state) => state.activeTab);
export const useSidebarOpen = () => useStore((state) => state.sidebarOpen);
export const useNotifications = () => useStore((state) => state.notifications);
export const useOverlayData = () => useStore((state) => state.overlayData);
export const useNotify = () => useStore((state) => state.notify);