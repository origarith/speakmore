# Release Readiness

This checklist tracks the work required after source publication and before SpeakMore publishes signed binary releases.

## Current State

- The source-first public repository is live.
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
- Public source publication is complete without binary artifacts, tags, or GitHub releases.
- The `main` ruleset blocks branch deletion, blocks non-fast-forward pushes, and requires pull requests.
- Audio feedback assets have been verified against the MIT-licensed upstream Handy repository. See [asset-provenance.md](asset-provenance.md).

## P0 Source Release Follow-Ups

- Verify one completed public `nix build check` run on `main`.
- After check names are stable, add required status checks to the `main` ruleset. Current observed check-run names are `code-quality`, `rust-tests`, `nix-build`, and `playwright`.
- Keep source availability separate from binary release work: do not create tags, GitHub releases, or installers until the signed binary release gates below are complete.
- Confirm SpeakMore-owned icon and tray image ownership before signed binary release. Current asset hashes and status are tracked in [asset-provenance.md](asset-provenance.md).
- Keep Qwen3-ASR ModelScope ONNX catalog entries experimental until SHA-256 checksums and artifact provenance are pinned.

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
- Record project-owned app icon and tray image ownership confirmation or replace them with assets that have documented third-party licensing.
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

- Creating Apple Developer credentials and certificates.
- Creating Azure Trusted Signing resources.
- Generating and safely storing updater private keys.
- Publishing a GitHub release after manual package smoke testing.
