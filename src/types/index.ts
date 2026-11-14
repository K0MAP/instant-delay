// TypeScript types matching Rust IPC types

export type DelayMode = 'instant' | 'normal' | 'disabled';

export interface PlatformOutput {
  id: string;
  name: string;
  platform: Platform;
  url: string;
  stream_key: string;
  enabled: boolean;
  priority: number;
  backup: boolean;
}

export type Platform = 'twitch' | 'youtube' | 'kick' | 'tiktok' | 'restream' | 'custom';

export interface HotkeyConfig {
  toggle_delay?: string;
  increase_delay?: string;
  decrease_delay?: string;
  skip_to_live?: string;
  toggle_video?: string;
  toggle_audio?: string;
}

export interface OverlayTheme {
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

export interface Settings {
  // Server
  bind_address: string;
  port: number;
  auto_start_server: boolean;

  // Delay
  default_delay_sec: number;
  max_delay_sec: number;
  delay_mode: DelayMode;
  delay_step_sec: number;

  // Buffer
  buffer_max_memory_mb: number;
  buffer_keyframe_only: boolean;

  // Platforms
  platform_outputs: PlatformOutput[];

  // Overlay
  overlay_enabled: boolean;
  overlay_port: number;
  overlay_theme: OverlayTheme;

  // Hotkeys
  hotkeys: HotkeyConfig;
  global_hotkeys_enabled: boolean;

  // Stream Deck
  streamdeck_enabled: boolean;
  streamdeck_port: number;

  // Web UI
  web_ui_enabled: boolean;
  web_ui_port: number;
  web_ui_auth_token?: string;

  // Advanced
  log_level: string;
  minimize_to_tray: boolean;
  start_minimized: boolean;
  check_updates: boolean;
}

export interface SystemInfo {
  os: string;
  arch: string;
  cpu_cores: number;
  total_memory_mb: number;
  available_memory_mb: number;
  obs_running: boolean;
  obs_version?: string;
}

export interface ServerStatusResponse {
  running: boolean;
  bind_address: string;
  port: number;
  active_connections: number;
  total_bytes_in: number;
  total_bytes_out: number;
  uptime_sec: number;
  current_delay_sec: number;
  delay_mode: DelayMode;
  is_buffering: boolean;
  buffer_fill_percent: number;
  platform_outputs: PlatformOutputStatus[];
}

export interface PlatformOutputStatus {
  id: string;
  name: string;
  platform: Platform;
  connected: boolean;
  bytes_sent: number;
  uptime_sec: number;
  last_error?: string;
}

export interface OverlayData {
  delay_sec: number;
  delay_mode: DelayMode;
  is_buffering: boolean;
  buffer_fill_percent: number;
  status_text: string;
  status_color: string;
  platform_count: number;
  connected_platforms: number;
}

export interface StreamDeckEvent {
  event: 'delay_changed' | 'mode_changed' | 'buffering_changed' | 'platform_status_changed' | 'error';
  data: any;
}

// Tauri command types
export interface TauriCommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// WebSocket message types
export type WSMessageType =
  | 'status_update'
  | 'delay_changed'
  | 'mode_changed'
  | 'buffering_changed'
  | 'platform_status'
  | 'error'
  | 'log';

export interface WSMessage<T = any> {
  type: WSMessageType;
  payload: T;
  timestamp: number;
}

// Component prop types
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'success';
  size?: 'sm' | 'md' | 'lg';
  loading?: boolean;
  icon?: React.ReactNode;
  iconPosition?: 'left' | 'right';
}

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helperText?: string;
  icon?: React.ReactNode;
}

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  options: { value: string; label: string }[];
}

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'hover' | 'glass';
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info';
  size?: 'sm' | 'md';
  dot?: boolean;
}