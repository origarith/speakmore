# Model Sources

SpeakMore uses a small bundled VAD model and a model catalog for optional ASR downloads. This document records where model artifacts come from and what must be checked before signed binary release packaging.

## Bundled Model Files

| File                                            | Purpose                    | Source note                                                                                                          |
| ----------------------------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/resources/models/silero_vad_v4.onnx` | Voice Activity Detection   | Silero VAD project. The upstream Silero VAD repository is MIT licensed.                                              |
| `src-tauri/resources/models/gigaam_vocab.txt`   | GigaAM vocabulary metadata | Same tracked artifact as upstream Handy. Upstream attribution still needs confirmation before signed binary release. |
| `src-tauri/resources/models/catalog.json`       | Model catalog metadata     | SpeakMore catalog file. It references remote model artifacts.                                                        |

## Catalog Sources

The catalog uses source labels:

- `Handy`: model artifact is served from the Handy model mirror.
- `SpeakMore`: model artifact or metadata was added by SpeakMore.

The Handy mirror base URL used by the model manager is:

```text
https://blob.handy.computer
```

Catalog entries include filenames, download paths, sizes, engine type, and checksums where available.

Qwen3-ASR catalog entries are directory models with multiple downloaded files. Their per-file URLs, sizes, and SHA-256 checksums are tracked in `src-tauri/resources/models/qwen3-asr-onnx-parts.json` and are checked by the source-release preflight. Treat those entries as experimental for release-quality packaging until the ModelScope revision, converted artifact provenance, and licensing are pinned.

## Known Upstream Projects

These upstream projects are directly relevant to the current model and ASR stack:

| Project            | Role                                  | License note                                                                                         |
| ------------------ | ------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Handy              | Upstream desktop app and model mirror | MIT                                                                                                  |
| OpenAI Whisper     | Whisper model family                  | MIT for code and model weights                                                                       |
| whisper.cpp / ggml | Local Whisper inference               | MIT                                                                                                  |
| Silero VAD         | Voice Activity Detection              | MIT                                                                                                  |
| NVIDIA Parakeet    | Parakeet ASR models                   | Check the specific model card. Parakeet TDT 0.6B v3 is published under CC BY 4.0.                    |
| Qwen3-ASR          | ASR model family                      | Qwen3-ASR project is Apache-2.0. Verify separately for converted ONNX artifacts and hosting mirrors. |

## Manual Model Installation

Users can place downloaded models in the app data `models` directory.

Typical paths:

- macOS: `~/Library/Application Support/app.speakmore.desktop/models/`
- Windows: `C:\Users\{username}\AppData\Roaming\app.speakmore.desktop\models\`
- Linux: `~/.config/app.speakmore.desktop/models/`

Whisper `.bin` files should keep their catalog filenames. Directory-based models should keep the directory names expected by the catalog.

## Release Checklist

Before shipping public binary releases:

1. Confirm every bundled model file has a redistributable license.
2. Add required attribution text to [NOTICE.md](../NOTICE.md).
3. Verify catalog checksums or multipart manifests for downloadable artifacts.
4. Avoid bundling large optional ASR models unless their licenses and sizes are intentional.
5. Document cloud provider terms separately from local model licenses.
6. Confirm model downloads do not require hidden credentials.
7. Confirm project-owned icons and tray images are covered by repository licensing or by a documented third-party license. Current asset status is tracked in [asset-provenance.md](asset-provenance.md).

This document is not legal advice. Treat it as the engineering checklist for license and source review.
