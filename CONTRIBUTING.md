# Contributing to SpeakMore

Thanks for helping improve SpeakMore. This project is preparing for its first public source release, so contributions that improve build reliability, documentation, packaging, privacy boundaries, and core speech-to-text stability are especially useful.

## Before You Start

- Search existing issues and pull requests to avoid duplicate work.
- Open an issue before starting broad behavior changes, new provider integrations, large UI changes, or release workflow changes.
- Keep pull requests focused. A small, tested fix is easier to review than a mixed cleanup and feature branch.
- Do not include provider API keys, personal audio, transcript history, or generated local logs in issues or PRs.

## Development Setup

Install:

- [Rust](https://rustup.rs/) latest stable
- [Bun](https://bun.sh/)
- Platform dependencies from [BUILD.md](BUILD.md)

Then:

```bash
bun install
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
bun run tauri dev
```

## Project Structure

Backend:

- `src-tauri/src/lib.rs`: Tauri setup and manager initialization
- `src-tauri/src/managers/`: audio, model, transcription, and history managers
- `src-tauri/src/audio_toolkit/`: lower-level audio and VAD utilities
- `src-tauri/src/commands/`: Tauri command handlers
- `src-tauri/src/settings.rs`: settings model and persistence
- `src-tauri/src/shortcut.rs`: global shortcut handling

Frontend:

- `src/App.tsx`: main settings/onboarding app shell
- `src/components/`: settings, onboarding, overlay, model selector, shared UI
- `src/stores/`: Zustand stores
- `src/i18n/`: i18next configuration and locale files
- `src/bindings.ts`: generated Tauri/Specta bindings

## Checks Before Opening a PR

Run the relevant checks for your change:

```bash
bun run lint
bun run build
bun run check:translations
cd src-tauri && cargo check
```

For formatting:

```bash
bun run format:check
```

For Rust tests when touching backend behavior:

```bash
cd src-tauri && cargo test
```

Some transcription, shortcut, audio-device, and packaging changes still need manual platform testing. Include the platform you tested in the PR description.

## Code Style

Rust:

- Run `cargo fmt`.
- Prefer explicit error handling over `unwrap` in production code.
- Add doc comments for public APIs.
- Keep platform-specific behavior isolated where possible.

TypeScript and React:

- Use strict TypeScript and avoid `any`.
- Keep components small and focused.
- Use existing settings/store patterns before adding new state machinery.
- All user-facing strings must use i18next translation keys.

Documentation:

- Keep public docs current and factual.
- Avoid internal paths, private project names, local machine details, and unpublished release promises.
- Link to specific docs instead of duplicating long setup sections.

## Translations

See [CONTRIBUTING_TRANSLATIONS.md](CONTRIBUTING_TRANSLATIONS.md). When adding a user-facing string, update the English source file and keep locale keys complete.

## AI Assistance

AI-assisted contributions are allowed. Disclose the tools used and the scope of assistance in the PR template so reviewers know what to inspect more closely.

Do not paste private transcripts, provider credentials, generated logs with secrets, or local-only paths into prompts or public PR text.

## Pull Request Guidelines

A good PR includes:

- A short description of what changed and why
- Linked issues when applicable
- The commands you ran
- Manual test notes for platform-specific behavior
- Screenshots or short videos for visible UI changes
- A clear note if AI assistance was used

Use conventional commit prefixes when practical:

- `fix:`
- `feat:`
- `docs:`
- `refactor:`
- `test:`
- `chore:`

## Security

Do not report vulnerabilities in public issues. See [SECURITY.md](SECURITY.md).

## License

By contributing to SpeakMore, you agree that your contributions are licensed under the MIT License. See [LICENSE](LICENSE).
