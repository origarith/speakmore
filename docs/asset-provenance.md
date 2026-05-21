# Asset Provenance

This document records the current provenance status for non-code assets that are tracked in the source tree. It is intended to keep source publication separate from signed binary release readiness.

## Summary

- Audio feedback sounds match the MIT-licensed upstream Handy repository.
- SpeakMore icon and tray images do not match upstream Handy assets and are treated as SpeakMore-owned project assets.
- No splash image is currently tracked in this repository.
- Signed binary releases should not ship until the project-owned app icon and tray image source files are retained or maintainer ownership is explicitly confirmed.

## Verification

Verification date: 2026-05-21.

Upstream reference:

- Repository: [cjpais/Handy](https://github.com/cjpais/Handy)
- License: MIT
- Commit checked: `e3206aa5725934fdd2fde1d6bfc80193608a1cbf`

## Audio Feedback

These files match the upstream Handy files byte-for-byte at the commit above.

| File                                    | SHA-256                                                            | Status                     |
| --------------------------------------- | ------------------------------------------------------------------ | -------------------------- |
| `src-tauri/resources/marimba_start.wav` | `ec58fb613b5102eca54689a6b42c389f1aa46154db80c27fea756879dfc6f39a` | Matches Handy MIT upstream |
| `src-tauri/resources/marimba_stop.wav`  | `682f408a066eadcccd5bc09e4055629bdd985f7ef222954a34f7e05ca1a456b2` | Matches Handy MIT upstream |
| `src-tauri/resources/pop_start.wav`     | `18dcc6fdeab5889f2e216da45d29c2df0a94f006fff6cfaee352f0df52a4779b` | Matches Handy MIT upstream |
| `src-tauri/resources/pop_stop.wav`      | `3c6dd27332c8613f533824979c470d32a62c326cdac35f1edea668602430d90d` | Matches Handy MIT upstream |

## Tray Images

These files are SpeakMore-specific and do not match the corresponding Handy upstream files. They are release-ready only after project ownership is confirmed.

| File                                             | SHA-256                                                            | Status                                         |
| ------------------------------------------------ | ------------------------------------------------------------------ | ---------------------------------------------- |
| `src-tauri/resources/speakmore.png`              | `472175dd38afd5deebd16ae4ee4938ec1ba4c5f5693ee5913c463659abdd46c7` | Project-owned, ownership confirmation required |
| `src-tauri/resources/recording.png`              | `2c3dbe7d3dce696e11153dd2910b661b00cab949c6dd108d2c0e0761a8ddb47a` | Project-owned, ownership confirmation required |
| `src-tauri/resources/transcribing.png`           | `5e1f47e12617f531255abbc0b778f899eb3a6173150d7677ae14c24beb8b3533` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_idle.png`              | `df54b0866599a9694b2b076f8dabd7e41102a0c9d36e42cb8f92e2e41815511e` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_idle_dark.png`         | `6b48fa58fa1a5ebdc9417caabca76a77f2dfbf5f984a2fb2f42041021e747dab` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_recording.png`         | `418434308859b5ebc79ebbf4d56a5d30506b1728b7ec09a78afbbf5fe537c294` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_recording_dark.png`    | `a1af8c66b3d75f726d26219763fe4e40a1f1189066e801adcfb30fa4022411fd` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_transcribing.png`      | `c669c9c2a94a5fd43884bcf4f3ec85bdfe46461ae5846c4075b04a0874381dc4` | Project-owned, ownership confirmation required |
| `src-tauri/resources/tray_transcribing_dark.png` | `d7c166cc61067a8e02de62aa775be0a44c515123d212e79bab1d1c1cb73fd8c8` | Project-owned, ownership confirmation required |

## App Icons

The generated app icon set under `src-tauri/icons/` is SpeakMore-specific and does not match upstream Handy. The files are treated as generated derivatives of the SpeakMore project mark, but binary release readiness still requires retaining the editable source or recording maintainer ownership confirmation.

Representative root icon hashes:

| File                        | SHA-256                                                            | Status                                         |
| --------------------------- | ------------------------------------------------------------------ | ---------------------------------------------- |
| `src-tauri/icons/logo.png`  | `19b3804f19ac60412e92cf37df7fcdbaf60725f2790dd9a3f8c70fc282e9a699` | Project-owned, ownership confirmation required |
| `src-tauri/icons/icon.png`  | `b83f43daf429ea50ed4e8769adcdc7a14b31e40b2a11fa12d4b33db082f1d530` | Project-owned, ownership confirmation required |
| `src-tauri/icons/icon.icns` | `115fd6ed032003f248c271114e7ae8f0fa1beb7276f5c833ce266d9643364af2` | Project-owned, ownership confirmation required |
| `src-tauri/icons/icon.ico`  | `5d482537d68a712f96b8ec31e78255baffa5f9320231b5e03fa72c3ceb863f21` | Project-owned, ownership confirmation required |

The platform-size PNG files under `src-tauri/icons/`, `src-tauri/icons/android/`, and `src-tauri/icons/ios/` should be regenerated from the same confirmed source before the first signed binary release.
