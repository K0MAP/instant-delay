import {
  Zap,
  Github,
  ExternalLink,
  Heart,
  Coffee,
  Code,
  Server,
  Monitor,
  Keyboard,
  Wifi,
  Shield,
  Rocket,
  Layers,
  BookOpen,
  MessageSquare,
  AlertTriangle,
  CheckCircle,
  X,
  Clock,
  Users,
  Sun,
  Moon
} from 'lucide-react';
import { useTheme } from '../hooks/useTheme';
import clsx from 'clsx';

const FEATURES = [
  { icon: Zap, title: 'Instant Delay Toggle', desc: 'Switch between live and delayed modes instantly without restarting OBS' },
  { icon: Clock, title: 'Configurable Delay', desc: 'Set delay from 1 to 600 seconds with real-time adjustment' },
  { icon: Server, title: 'Multi-Platform Relay', desc: 'Stream to Twitch, YouTube, Kick, TikTok, Restream, and custom RTMP simultaneously' },
  { icon: Monitor, title: 'LIVE/DELAYED Overlay', desc: 'Browser source overlay for OBS showing real-time status to viewers' },
  { icon: Keyboard, title: 'Global Hotkeys', desc: 'Control delay from anywhere with customizable keyboard shortcuts' },
  { icon: Wifi, title: 'System Tray', desc: 'Run minimized in background with quick access from system tray' },
  { icon: Shield, title: 'Stream Deck Support', desc: 'Native Stream Deck plugin with actions and real-time feedback' },
  { icon: Rocket, title: 'High Performance', desc: 'Built with Rust and Tauri for minimal resource usage' },
];

const TECH_STACK = [
  { name: 'Rust', icon: Server, color: 'text-orange-500', desc: 'Backend RTMP server and core logic' },
  { name: 'Tauri 2.0', icon: Monitor, color: 'text-blue-500', desc: 'Native desktop app framework' },
  { name: 'React 18', icon: Code, color: 'text-cyan-400', desc: 'Modern UI with hooks and context' },
  { name: 'TypeScript', icon: Code, color: 'text-blue-400', desc: 'Type-safe development' },
  { name: 'Tokio', icon: Server, color: 'text-green-500', desc: 'Async runtime for high concurrency' },
  { name: 'Axum', icon: Wifi, color: 'text-emerald-400', desc: 'Web framework for REST API and WebSocket' },
  { name: 'Zustand', icon: Layers, color: 'text-yellow-400', desc: 'Lightweight state management' },
  { name: 'Tailwind CSS', icon: Monitor, color: 'text-cyan-500', desc: 'Utility-first styling' },
];

type ComparisonValue = boolean | 'partial' | 'varies';

const COMPARISON: { feature: string; instantDelay: boolean; obsBuiltIn: boolean; otherTools: ComparisonValue }[] = [
  { feature: 'Instant delay toggle', instantDelay: true, obsBuiltIn: false, otherTools: 'partial' },
  { feature: 'No OBS restart required', instantDelay: true, obsBuiltIn: false, otherTools: true },
  { feature: 'Multi-platform relay', instantDelay: true, obsBuiltIn: false, otherTools: 'partial' },
  { feature: 'LIVE/DELAYED overlay', instantDelay: true, obsBuiltIn: false, otherTools: false },
  { feature: 'Global hotkeys', instantDelay: true, obsBuiltIn: false, otherTools: true },
  { feature: 'System tray', instantDelay: true, obsBuiltIn: false, otherTools: true },
  { feature: 'Stream Deck plugin', instantDelay: true, obsBuiltIn: false, otherTools: 'partial' },
  { feature: 'Custom RTMP outputs', instantDelay: true, obsBuiltIn: true, otherTools: true },
  { feature: 'Open source', instantDelay: true, obsBuiltIn: true, otherTools: 'varies' },
  { feature: 'Cross-platform', instantDelay: true, obsBuiltIn: true, otherTools: 'varies' },
];

export function About() {
  const { theme, toggleTheme } = useTheme();

  const renderComparisonIcon = (value: boolean | 'partial' | 'varies') => {
    if (value === true) return <CheckCircle className="w-5 h-5 text-green-400" />;
    if (value === 'partial') return <AlertTriangle className="w-5 h-5 text-yellow-400" />;
    if (value === 'varies') return <AlertTriangle className="w-5 h-5 text-yellow-400" />;
    return <X className="w-5 h-5 text-red-400" />;
  };

  return (
    <div className="space-y-6 animate-in">
      {/* Hero Section */}
      <div className="card p-8 md:p-12 text-center relative overflow-hidden">
        <div className="absolute inset-0 bg-gradient-to-br from-primary-500/10 via-transparent to-transparent" />
        <div className="relative z-10">
          <div className="w-24 h-24 rounded-2xl bg-gradient-to-br from-primary-500 to-primary-600 flex items-center justify-center mx-auto mb-6">
            <Zap className="w-12 h-12 text-white" />
          </div>
          <h1 className="text-4xl md:text-5xl font-bold text-surface-50 mb-4">InstantDelay</h1>
          <p className="text-xl text-surface-300 max-w-2xl mx-auto mb-8">
            Professional stream delay management for OBS. Control your stream delay in real-time
            without restarting OBS. Built for streamers who need precision and reliability.
          </p>
          <div className="flex flex-wrap items-center justify-center gap-4">
            <a href="https://github.com/yourname/instant-delay" target="_blank" rel="noopener noreferrer" className="btn-primary flex items-center gap-2">
              <Github className="w-5 h-5" />
              View on GitHub
            </a>
            <a href="https://github.com/yourname/instant-delay/releases" target="_blank" rel="noopener noreferrer" className="btn-secondary flex items-center gap-2">
              <Rocket className="w-5 h-5" />
              Download Latest
            </a>
            <button onClick={toggleTheme} className="btn-ghost flex items-center gap-2">
              {theme === 'dark' ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
              {theme === 'dark' ? 'Light Mode' : 'Dark Mode'}
            </button>
          </div>
        </div>
      </div>

      {/* Key Features */}
      <div>
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-surface-50">Key Features</h2>
            <p className="text-surface-400">Everything you need to manage stream delay professionally</p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {FEATURES.map((feature, index) => {
            const Icon = feature.icon;
            return (
              <div key={index} className="card p-6 hover:shadow-[0_0_30px_rgba(255,107,53,0.1)] transition-shadow duration-300">
                <div className="w-12 h-12 rounded-xl bg-primary-500/10 flex items-center justify-center mb-4">
                  <Icon className="w-6 h-6 text-primary-400" />
                </div>
                <h3 className="text-lg font-semibold text-surface-50 mb-2">{feature.title}</h3>
                <p className="text-surface-400 text-sm">{feature.desc}</p>
              </div>
            );
          })}
        </div>
      </div>

      {/* Tech Stack */}
      <div>
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-surface-50">Technology Stack</h2>
            <p className="text-surface-400">Built with modern, performant technologies</p>
          </div>
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {TECH_STACK.map((tech, index) => (
            <div key={index} className="card p-4">
              <div className="flex items-center gap-3 mb-3">
                <tech.icon className={clsx('w-6 h-6', tech.color)} />
                <span className="font-semibold text-surface-50">{tech.name}</span>
              </div>
              <p className="text-sm text-surface-400">{tech.desc}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Comparison Table */}
      <div>
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-surface-50">Why InstantDelay?</h2>
            <p className="text-surface-400">Comparison with built-in OBS delay and other tools</p>
          </div>
        </div>
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-surface-800">
                  <th className="p-4 text-left text-sm font-medium text-surface-400">Feature</th>
                  <th className="p-4 text-center text-sm font-medium text-primary-400">InstantDelay</th>
                  <th className="p-4 text-center text-sm font-medium text-surface-400">OBS Built-in</th>
                  <th className="p-4 text-center text-sm font-medium text-surface-400">Other Tools</th>
                </tr>
              </thead>
              <tbody>
                {COMPARISON.map((row, index) => (
                  <tr key={index} className="border-b border-surface-800/50 hover:bg-surface-800/50">
                    <td className="p-4 text-sm text-surface-300">{row.feature}</td>
                    <td className="p-4 text-center">{renderComparisonIcon(row.instantDelay)}</td>
                    <td className="p-4 text-center">{renderComparisonIcon(row.obsBuiltIn)}</td>
                    <td className="p-4 text-center">{renderComparisonIcon(row.otherTools)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="p-4 border-t border-surface-800 flex items-center gap-4 text-xs text-surface-500">
            <span className="flex items-center gap-1"><CheckCircle className="w-3 h-3 text-green-400" /> Full support</span>
            <span className="flex items-center gap-1"><AlertTriangle className="w-3 h-3 text-yellow-400" /> Partial/Limited</span>
            <span className="flex items-center gap-1"><X className="w-3 h-3 text-red-400" /> Not supported</span>
          </div>
        </div>
      </div>

      {/* How It Works */}
      <div>
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-surface-50">How It Works</h2>
            <p className="text-surface-400">Simple architecture for reliable stream delay</p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {[
            { step: 1, title: 'Configure OBS', desc: 'Set your RTMP destination to localhost:1935 (InstantDelay server)' },
            { step: 2, title: 'Set Delay', desc: 'Choose your delay time (1-600s) and mode (Instant/Normal/Disabled)' },
            { step: 3, title: 'Go Live', desc: 'Start streaming in OBS - InstantDelay handles the rest automatically' },
          ].map((item) => (
            <div key={item.step} className="card p-6 relative">
              <div className="absolute -top-3 left-6 w-8 h-8 rounded-full bg-primary-500 flex items-center justify-center text-white font-bold">
                {item.step}
              </div>
              <div className="pt-4">
                <h3 className="text-lg font-semibold text-surface-50 mb-2">{item.title}</h3>
                <p className="text-surface-400 text-sm">{item.desc}</p>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Use Cases */}
      <div>
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-surface-50">Perfect For</h2>
            <p className="text-surface-400">Streamers who need professional delay management</p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {[
            { icon: Shield, title: 'Anti-Sniping', desc: 'Prevent stream snipers in competitive games with configurable delay' },
            { icon: Users, title: 'Co-Streaming', desc: 'Sync multiple streamers with different delay requirements' },
            { icon: Monitor, title: 'Tournament Production', desc: 'Production-grade delay for esports events and tournaments' },
            { icon: MessageSquare, title: 'Chat Interaction', desc: 'Read and respond to chat while maintaining stream delay' },
            { icon: BookOpen, title: 'Educational Content', desc: 'Delay sensitive content or spoilers for educational streams' },
            { icon: Coffee, title: 'Break Management', desc: 'Instantly switch to delayed mode during breaks without restarting' },
          ].map((item, index) => (
            <div key={index} className="card p-6">
              <div className="w-10 h-10 rounded-lg bg-primary-500/10 flex items-center justify-center mb-4">
                <item.icon className="w-5 h-5 text-primary-400" />
              </div>
              <h3 className="text-lg font-semibold text-surface-50 mb-2">{item.title}</h3>
              <p className="text-surface-400 text-sm">{item.desc}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Links & Resources */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <a href="https://github.com/yourname/instant-delay" target="_blank" rel="noopener noreferrer" className="card p-6 hover:border-primary-500/50 transition-colors">
          <div className="flex items-center gap-3 mb-3">
            <Github className="w-8 h-8 text-surface-400" />
            <h3 className="text-lg font-semibold text-surface-50">GitHub Repository</h3>
          </div>
          <p className="text-surface-400 text-sm">Source code, issues, releases, and contribution guidelines</p>
        </a>
        <a href="https://github.com/yourname/instant-delay/wiki" target="_blank" rel="noopener noreferrer" className="card p-6 hover:border-primary-500/50 transition-colors">
          <div className="flex items-center gap-3 mb-3">
            <BookOpen className="w-8 h-8 text-surface-400" />
            <h3 className="text-lg font-semibold text-surface-50">Documentation</h3>
          </div>
          <p className="text-surface-400 text-sm">Setup guides, API reference, and advanced configuration</p>
        </a>
        <a href="https://github.com/yourname/instant-delay/discussions" target="_blank" rel="noopener noreferrer" className="card p-6 hover:border-primary-500/50 transition-colors">
          <div className="flex items-center gap-3 mb-3">
            <MessageSquare className="w-8 h-8 text-surface-400" />
            <h3 className="text-lg font-semibold text-surface-50">Community</h3>
          </div>
          <p className="text-surface-400 text-sm">Discussions, feature requests, and community support</p>
        </a>
      </div>

      {/* License & Credits */}
      <div className="card p-6 border-t border-surface-800">
        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <Heart className="w-5 h-5 text-red-500" />
              <span className="font-medium text-surface-300">Made with</span>
            </div>
            <div className="flex items-center gap-2">
              <Coffee className="w-5 h-5 text-amber-500" />
              <span className="font-medium text-surface-300">by streamers</span>
            </div>
            <div className="flex items-center gap-2">
              <Code className="w-5 h-5 text-primary-500" />
              <span className="font-medium text-surface-300">for streamers</span>
            </div>
          </div>
          <div className="flex items-center gap-6 text-sm text-surface-500">
            <span>MIT Licensed</span>
            <span>v1.0.0</span>
            <a href="https://github.com/yourname/instant-delay/blob/main/LICENSE" target="_blank" rel="noopener noreferrer" className="hover:text-primary-400 transition-colors">
              View License
              <ExternalLink className="w-3 h-3 ml-1 inline" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}