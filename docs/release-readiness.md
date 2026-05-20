# Release Readiness

This checklist tracks the work required before SpeakMore is opened publicly or publishes signed binary releases.

## Current State

- Source builds are supported for development.
- GitHub release publishing exists as a workflow, but signed public release is intentionally blocked by release preflight until updater and signing configuration are complete.
- Tauri updater artifacts are disabled in `src-tauri/tauri.conf.json`.
- The online repository currently has no GitHub releases or version tags.
- The current release matrix is narrower than upstream Handy: Apple Silicon macOS, Linux x64, Linux arm64, and Windows x64.

## Done In Repository

- CI is split into code quality, Rust tests, Nix checks, Playwright, main branch packaging, PR test packaging, and release workflows.
- Unsigned PR/main packaging can run without Apple, Azure, or Tauri updater signing secrets.
- Signed release workflow now has preflight checks before creating a draft release.
- Release preflight checks version consistency across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- Release preflight blocks signed releases until updater artifacts, updater metadata, Windows signing, and signing secrets are configured.
- Issue templates and PR template include release and packaging impact prompts.

## P0 Before Public Source Release

- Resolve the GitHub Actions billing or spending-limit block on `OrigArith/SpeakMore`.
- Re-run required PR checks after billing is fixed.
- Merge the CI stabilization PR only after `code quality`, `test`, `nix build check`, and manual PR packaging pass.
- Keep the repository private until source audit is complete.
- Review `LICENSE`, `NOTICE.md`, bundled assets, model source notes, and third-party attribution.
- Confirm SpeakMore-owned icon and tray assets are safe to publish under the repository license.
- Run `bun run preflight:source-release` on a clean worktree before changing the repository to public.
- Decide whether the first public release is source-only or includes signed binaries.
- Enable GitHub Discussions before relying on discussion contact links from issue templates.
- Add repository topics such as `speech-to-text`, `tauri-v2`, `accessibility`, and `cross-platform`.

## P0 Before Signed Binary Release

- Generate a Tauri updater signing keypair.
- Store `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as GitHub Actions secrets.
- Add the updater public key and GitHub `latest.json` endpoint to `src-tauri/tauri.conf.json`.
- Set `bundle.createUpdaterArtifacts` to `true`.
- Configure Apple Developer ID signing and notarization secrets:
  - `APPLE_ID`
  - `APPLE_ID_PASSWORD`
  - `APPLE_PASSWORD`
  - `APPLE_TEAM_ID`
  - `APPLE_CERTIFICATE`
  - `APPLE_CERTIFICATE_PASSWORD`
  - `KEYCHAIN_PASSWORD`
- Configure Windows Trusted Signing secrets:
  - `AZURE_CLIENT_ID`
  - `AZURE_CLIENT_SECRET`
  - `AZURE_TENANT_ID`
- Add a SpeakMore-specific `bundle.windows.signCommand` to `src-tauri/tauri.conf.json`.
- Pin SHA-256 checksums or remove release-quality claims for ModelScope-hosted Qwen3-ASR catalog artifacts.
- Run the release workflow and confirm the draft release contains binaries, `.sig` files, and `latest.json`.
- Install and smoke test each uploaded package on its target OS before publishing the draft release.

## P1 Release Matrix Decisions

- Decide whether to restore macOS Intel release builds.
- Decide whether to restore Windows ARM64 release builds.
- Document unsupported or best-effort platforms in README before the first public release.
- If a platform is added back, require one successful PR test package and one manual install smoke test before publishing it.

## P1 Repository Governance

- Configure branch protection or repository rulesets for `main`.
- Require at least the CI checks that gate source health.
- Require PR review once outside contributors are expected.
- Keep blank issues disabled so reports use structured templates.
- Send broad feature requests to Discussions before implementation PRs.

## P2 Package-Manager Distribution

- Publish GitHub releases first and keep at least one release stable before package-manager submissions.
- Prepare Homebrew cask metadata after signed macOS artifacts are stable.
- Prepare winget metadata after signed Windows artifacts are stable.
- Decide whether Scoop, Chocolatey, Flatpak, Snap, or Linux distro packaging is in scope.
- Document who owns package-manager updates if maintainers outside this repository submit them.

## External Blockers

These cannot be completed from the repository alone:

- Fixing GitHub billing or spending limit for Actions.
- Creating Apple Developer credentials and certificates.
- Creating Azure Trusted Signing resources.
- Generating and safely storing updater private keys.
- Changing repository visibility to public.
- Publishing a GitHub release after manual package smoke testing.
