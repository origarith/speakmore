# SpeakMore

SpeakMore is a cross-platform desktop speech-to-text app built with Tauri, Rust, React, and TypeScript. It focuses on a small workflow: press a shortcut, speak, turn audio into text, optionally clean it up, and paste the result into the active app.

SpeakMore is local-first. Local ASR keeps audio on your machine. Cloud ASR is available only when you configure a provider and explicitly choose that recognition path.

## Features

- Local speech recognition with Whisper and other bundled model families
- Optional cloud ASR providers for cases where remote recognition is preferred
- Voice Activity Detection using Silero VAD
- Configurable global shortcuts, including push-to-talk style workflows
- Optional post-processing presets for cleaning or formatting transcripts
- Local transcript history with retry, copy, and edit support
- macOS, Windows, and Linux builds from source

## Project Status

SpeakMore is preparing for its first public source release. The repository currently supports source builds and development workflows. Public binary releases, package-manager distribution, updater metadata, and signed release artifacts are not available yet.

Current priorities:

- Make the source tree easy to build and audit
- Stabilize Apple Silicon, Windows x64, and Linux source-build paths
- Keep cloud ASR explicit, configurable, and clearly separated from local mode
- Improve model source documentation and release packaging

Release readiness is tracked in [docs/release-readiness.md](docs/release-readiness.md).

## Quick Start

### Build From Source

See [BUILD.md](BUILD.md) for platform-specific prerequisites.

```bash
bun install
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
bun run tauri dev
```

On macOS, if CMake rejects an older dependency policy during setup, run:

```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev
```

### Development Checks

```bash
bun run lint
bun run build
bun run check:translations
cd src-tauri && cargo check
```

## How It Works

1. Press a configured shortcut to start recording.
2. Speak while recording is active.
3. SpeakMore filters silence and sends the captured audio to the selected ASR path.
4. SpeakMore returns the transcript to the active app through paste or typing.

Recognition can use:

- Local models, including Whisper-family models and other supported local ASR engines
- Cloud ASR providers, if configured in settings
- Realtime cloud preview where supported by the selected provider

Post-processing is separate from recognition. You can use the raw transcript directly, or run it through a preset that cleans dictation, formats text, or prepares a concise message.

## Privacy Model

SpeakMore has two different privacy modes:

- Local ASR: audio and transcripts stay on your machine unless you export or share them.
- Cloud ASR: audio is sent to the provider you configure. Provider credentials and network access are required.

SpeakMore should not write provider API keys, previous clipboard contents, or cloud provider secrets into transcript history or exported data. See [docs/history.md](docs/history.md) for the local history privacy boundary.

## Documentation

- [BUILD.md](BUILD.md): source build instructions
- [CONTRIBUTING.md](CONTRIBUTING.md): contribution workflow
- [CONTRIBUTING_TRANSLATIONS.md](CONTRIBUTING_TRANSLATIONS.md): translation guide
- [docs/asr-providers.md](docs/asr-providers.md): local and cloud ASR provider model
- [docs/post-processing.md](docs/post-processing.md): post-processing preset behavior
- [docs/history.md](docs/history.md): history storage and privacy boundary
- [docs/model-sources.md](docs/model-sources.md): model and third-party source notes
- [docs/release-readiness.md](docs/release-readiness.md): public release and signed binary checklist

## Architecture

SpeakMore combines a Rust backend with a React settings UI:

- `src-tauri/src/`: Tauri app, audio, shortcuts, history, settings, transcription, model management
- `src-tauri/src/audio_toolkit/`: device I/O, recording, resampling, VAD helpers
- `src-tauri/src/commands/`: Tauri command handlers
- `src/`: React UI, settings screens, onboarding, overlay, stores, and generated Tauri bindings
- `src/i18n/`: i18next setup and locale files

The core flow is:

```text
audio input -> VAD -> ASR provider -> transcript -> optional post-processing -> paste/copy/history
```

## CLI

SpeakMore supports command-line flags for controlling a running instance:

```bash
speakmore --toggle-transcription
speakmore --toggle-post-process
speakmore --cancel
speakmore --start-hidden
speakmore --no-tray
speakmore --debug
```

On macOS app bundles, invoke the binary directly:

```bash
/Applications/SpeakMore.app/Contents/MacOS/SpeakMore --toggle-transcription
```

## Platform Notes

### macOS

- Apple Silicon is the primary macOS target.
- Accessibility permissions are required for global shortcuts and text insertion.
- Intel macOS source builds are best effort.

### Windows

- Windows x64 is the primary Windows target.
- Windows ARM64 is experimental until runner and device coverage are reliable.

### Linux

- Linux x64 and arm64 source builds are supported targets.
- On X11, install `xdotool` for reliable text insertion.
- On Wayland, install `wtype` or configure desktop-level shortcuts that call the CLI flags.
- The recording overlay is disabled by default on Linux because some compositors may give it focus and interfere with paste behavior.

If startup fails with `libgtk-layer-shell.so.0`, install the runtime package:

| Distro        | Package               |
| ------------- | --------------------- |
| Ubuntu/Debian | `libgtk-layer-shell0` |
| Fedora/RHEL   | `gtk-layer-shell`     |
| Arch Linux    | `gtk-layer-shell`     |

Useful Linux workarounds:

```bash
SPEAKMORE_NO_GTK_LAYER_SHELL=1 speakmore
WEBKIT_DISABLE_DMABUF_RENDERER=1 speakmore
```

## Manual Model Installation

If automatic model downloads are blocked by a proxy or firewall, place model files in the app data `models` directory and restart SpeakMore.

Typical app data locations:

- macOS: `~/Library/Application Support/app.speakmore.desktop/`
- Windows: `C:\Users\{username}\AppData\Roaming\app.speakmore.desktop\`
- Linux: `~/.config/app.speakmore.desktop/`

For model source details, filenames, and attribution notes, see [docs/model-sources.md](docs/model-sources.md).

## Release Signatures

Tauri updater artifacts are currently disabled. SpeakMore does not yet publish updater signatures or a `latest.json` endpoint.

## Contributing

Issues and pull requests are welcome while the project is being prepared for public release. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

For security issues, do not open a public issue. See [SECURITY.md](SECURITY.md).

## License

SpeakMore is licensed under the MIT License. See [LICENSE](LICENSE).

This project is based on the MIT-licensed Handy project by CJ Pais. Additional third-party notices are documented in [NOTICE.md](NOTICE.md) and [docs/model-sources.md](docs/model-sources.md).

## Acknowledgments

- Handy, the original MIT-licensed upstream project
- OpenAI Whisper
- whisper.cpp and ggml
- Silero VAD
- Tauri
- The broader open-source speech recognition community
