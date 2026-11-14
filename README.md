# InstantDelay

**Change stream delay while live—without restarting your streaming setup.** InstantDelay is an experimental desktop application built with Tauri 2, React, TypeScript, and Rust. It is intended to sit between an OBS RTMP output and one or more streaming destinations, buffering media so the delay can be controlled from the app.

> **Status: experimental / work in progress.** The project builds and has unit tests, but the RTMP ingest/relay path has not been validated end-to-end against OBS and production streaming services. Do not rely on it as the only path for a live broadcast. Test with a private stream and keep a working fallback configuration.

[Українська версія нижче](#українською)

## Intended capabilities

- Local RTMP ingest and configurable delay buffer.
- Delay modes and controls for changing, increasing, decreasing, or skipping the delay.
- RTMP output profiles for Twitch, YouTube, Kick, TikTok, Restream, and custom RTMP endpoints.
- Local browser-source overlay with status updates over WebSocket.
- Global hotkey settings and a Stream Deck integration scaffold.
- Local settings for buffer size, overlay appearance, and server ports.

Feature availability is still evolving. In particular, a passing build or unit test does **not** imply that a complete live-stream workflow works with every encoder, server, or platform. The platform connection test currently validates configuration; it is not a complete end-to-end streaming test.

## Requirements

The desktop build is currently developed and verified on Windows. Other operating systems may require additional platform-specific work.

- Windows 10/11, with WebView2 Runtime available.
- Node.js and npm (an active LTS release is recommended).
- Rust stable toolchain with the MSVC target.
- Visual Studio Build Tools with the C++ build tools and Windows SDK.
- Git.

Tauri's official setup guide has the up-to-date platform prerequisites: <https://tauri.app/start/prerequisites/>.

## Build and run

From the repository root in PowerShell:

```powershell
npm ci
npm run tauri dev
```

Build the frontend:

```powershell
npm run build
```

Run the Rust unit tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Create a desktop release bundle:

```powershell
npm run tauri build
```

The installer/bundle is written under `src-tauri/target/release/bundle/`. Do not distribute a build until the target streaming workflow has been tested on the intended platform.

## Default local endpoints

| Service | Default address | Purpose |
| --- | --- | --- |
| RTMP ingest | `127.0.0.1:1935` | Local stream input |
| Overlay page and WebSocket | `127.0.0.1:8080` | OBS browser-source overlay |
| Web UI API | `127.0.0.1:8081` | Local application API |
| Stream Deck | `127.0.0.1:3000` | Optional integration setting |

The HTTP and overlay servers bind to loopback by default. Keep them local: do not port-forward them, expose them on a public interface, or put them behind a tunnel. The web API is not intended to be a hardened remote-control service; configure it only on a trusted machine.

## Project layout

```text
src/                 React/TypeScript user interface
src-tauri/src/       Tauri application and Rust services
  ipc/               Tauri commands, shared settings, and events
  rtmp/              RTMP protocol, server, relay, sessions, and buffer
  web/               Local API and overlay HTTP/WebSocket servers
  overlay/           Overlay HTML generation
  hotkeys/           Global shortcut integration
  streamdeck/        Stream Deck integration
  config/            Settings persistence
```

## Security and stream keys

- Treat RTMP stream keys and web-control tokens as secrets. Never commit real keys, local settings, screenshots containing credentials, or `.env` files.
- Store real platform credentials only in the app's local settings. Use test credentials or private test streams during development.
- The default servers use loopback addresses; do not change that assumption casually. Remote access needs authentication, authorization, and a separate security review.
- Review [SECURITY.md](SECURITY.md) before reporting a vulnerability.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development and pull-request checklist. Please include a clear reproduction for bugs and note which OS, Rust, Node.js, and OBS versions were tested.

## License

This project is distributed under the MIT License; see [LICENSE](LICENSE).

---

## Українською

**Змінюйте затримку стріму під час трансляції без перезапуску OBS.** InstantDelay — експериментальна desktop-програма на Tauri 2, React, TypeScript і Rust. Вона призначена працювати між RTMP-виходом OBS і платформами трансляції, буферизуючи медіадані для керування затримкою.

> **Статус: експериментальний проєкт у розробці.** Збірка та модульні тести працюють, але повний сценарій RTMP ingest/relay ще не перевірено від початку до кінця з OBS і реальними сервісами. Не використовуйте програму як єдиний шлях трансляції без тестування; залиште робочий резервний варіант.

### Можливості, що розробляються

- Локальний RTMP ingest і буфер із налаштовуваною затримкою.
- Режими затримки та керування затримкою з інтерфейсу.
- Профілі RTMP для Twitch, YouTube, Kick, TikTok, Restream і власних серверів.
- Локальний overlay для Browser Source в OBS із WebSocket-оновленнями.
- Налаштування глобальних гарячих клавіш і заготовка інтеграції Stream Deck.

Функціональність продовжує розроблятися. Успішна збірка чи модульний тест не гарантують повну роботу трансляції з кожним енкодером або платформою. Перевірка підключення наразі перевіряє конфігурацію, а не повний сценарій передавання стріму.

### Запуск у Windows

Потрібні Node.js/npm, стабільний Rust із MSVC, Visual Studio C++ Build Tools і Windows SDK, а також WebView2 Runtime. Деталі — в офіційному [посібнику Tauri](https://tauri.app/start/prerequisites/).

```powershell
npm ci
npm run tauri dev
```

Збірка інтерфейсу, Rust-тести та релізний пакет:

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

### Безпека

Не додавайте до Git справжні stream keys, токени чи локальні налаштування. HTTP-сервери за замовчуванням слухають лише loopback. Не відкривайте їх у мережу, не налаштовуйте port forwarding або тунель без окремого захисту й аудиту.
