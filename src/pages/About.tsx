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
  Shield,
  Rocket,
  BookOpen,
  MessageSquare,
  Clock,
  Users,
  Sun,
  Moon
} from 'lucide-react';
import { useTheme } from '../hooks/useTheme';

const FEATURES = [
  { icon: Zap, title: 'Delay Controls', desc: 'Adjust, pause, or bypass the configured stream delay from one place' },
  { icon: Clock, title: 'Configurable Buffer', desc: 'Choose a delay and monitor the buffered media' },
  { icon: Server, title: 'RTMP Relay', desc: 'Forward a local RTMP input to configured destinations' },
  { icon: Monitor, title: 'Status Overlay', desc: 'Display the current live or delayed state in a browser source' },
  { icon: Keyboard, title: 'Hotkeys', desc: 'Configure keyboard shortcuts for common delay actions' },
  { icon: Rocket, title: 'System Tray', desc: 'Keep the control panel available while it runs in the background' },
  { icon: Shield, title: 'Local Controls', desc: 'Manage the stream and settings from the desktop application' },
  { icon: BookOpen, title: 'Open Development', desc: 'Review the source, project notes, and release status in the repository' },
];

export function About() {
  const { theme, toggleTheme } = useTheme();

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
            Experimental software for managing live stream delay. Configure an RTMP input, choose a delay,
            and relay the buffered stream to your selected destinations.
          </p>
          <div className="flex flex-wrap items-center justify-center gap-4">
            <a href="https://github.com/K0MAP/instant-delay" target="_blank" rel="noopener noreferrer" className="btn-primary flex items-center gap-2">
              <Github className="w-5 h-5" />
              View on GitHub
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
            <p className="text-surface-400">Core controls for configuring and monitoring stream delay</p>
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
            { step: 1, title: 'Configure Input', desc: 'Point your encoder to the local RTMP endpoint' },
            { step: 2, title: 'Set Delay', desc: 'Choose the delay and operating mode in the application' },
            { step: 3, title: 'Start Streaming', desc: 'Begin the input stream and monitor the relay status' },
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
            <a href="https://github.com/K0MAP/instant-delay/blob/main/LICENSE" target="_blank" rel="noopener noreferrer" className="hover:text-primary-400 transition-colors">
              View License
              <ExternalLink className="w-3 h-3 ml-1 inline" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}