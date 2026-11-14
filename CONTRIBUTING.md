# Contributing

Thanks for helping improve InstantDelay. The project is experimental; please keep changes focused and describe clearly what has and has not been verified.

## Development setup

Install the prerequisites listed in [README.md](README.md), then from the repository root:

```powershell
npm ci
npm run tauri dev
```

## Checks

Run the relevant checks before opening a pull request:

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

If a change affects the Tauri desktop integration, also build it on the target OS. Clearly state if manual OBS/platform testing was not performed.

## Pull requests

- Explain the user-visible change and the motivation.
- Link related issues, when applicable.
- Include reproduction steps for bug fixes and screenshots for UI changes.
- Describe security implications when handling RTMP URLs, stream keys, settings, local HTTP endpoints, or IPC.
- Do not include credentials, real stream keys, user settings, build output, or unrelated formatting changes.
- Keep tests focused and update documentation when behavior or setup changes.
