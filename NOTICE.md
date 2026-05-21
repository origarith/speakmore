# Notices

SpeakMore is based on the MIT-licensed Handy project by CJ Pais.

The original Handy copyright notice is preserved in [LICENSE](LICENSE). SpeakMore modifications are also distributed under the MIT License unless a file states otherwise.

## Upstream Project

- Handy: https://github.com/cjpais/Handy
- License: MIT
- Role: original desktop speech-to-text application, architecture foundation, and model mirror lineage

## Third-Party Projects and Models

SpeakMore uses or integrates with the following projects and model families:

| Project            | Role                          | License note                                                                  |
| ------------------ | ----------------------------- | ----------------------------------------------------------------------------- |
| Tauri              | Desktop application framework | See upstream project license                                                  |
| React              | Frontend UI                   | MIT                                                                           |
| OpenAI Whisper     | ASR model family              | MIT for code and model weights                                                |
| whisper.cpp / ggml | Local Whisper inference       | MIT                                                                           |
| Silero VAD         | Voice Activity Detection      | MIT                                                                           |
| NVIDIA Parakeet    | ASR model family              | Check each model card. Parakeet TDT 0.6B v3 is published under CC BY 4.0.     |
| Qwen3-ASR          | ASR model family              | Qwen3-ASR project is Apache-2.0. Verify converted model artifacts separately. |
| sherpa-onnx        | ONNX ASR runtime support      | See upstream project license                                                  |
| transcribe-rs      | Local ASR runtime support     | See upstream project license                                                  |

Model source details and release checklist items are tracked in [docs/model-sources.md](docs/model-sources.md). Non-code asset provenance is tracked in [docs/asset-provenance.md](docs/asset-provenance.md).

## Release Packaging Note

The source tree includes `src-tauri/resources/models/silero_vad_v4.onnx` for Voice Activity Detection. Optional ASR models are normally downloaded at runtime from the configured catalog rather than committed to this repository.

The source tree also includes application icons, tray images, and audio feedback sounds. The audio feedback files match the upstream Handy assets. SpeakMore-specific icons and tray images are tracked as project-owned release assets and must have ownership confirmation recorded before signed binary release.

Before publishing binary releases, verify bundled model licenses, checksums, and attribution requirements for the exact artifacts included in the release.
