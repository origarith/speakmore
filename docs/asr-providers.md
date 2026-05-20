# ASR Providers

SpeakMore separates audio capture from speech recognition. The selected ASR provider receives prepared audio and returns transcript text plus metadata.

## Provider Modes

### Local ASR

Local ASR is the default privacy-preserving path. Audio is processed on the user's machine.

Supported local model families are defined in `src-tauri/resources/models/catalog.json` and loaded through the backend model manager. The catalog includes Whisper-family models, Parakeet-family models, SenseVoice, GigaAM, Canary, Cohere, and Qwen3-ASR ONNX entries.

### Cloud ASR

Cloud ASR is optional. It requires provider configuration, network access, and provider credentials. When a cloud provider is selected, audio is sent to that provider for recognition.

Cloud ASR must remain explicit in UI and documentation. It should not be described as offline or local.

## Provider Boundary

An ASR provider is responsible for:

- Accepting prepared audio
- Applying provider-specific language or model options
- Returning final transcript text
- Returning status, provider id, model id, language, latency, and error summaries

The caller is responsible for:

- Recording and stopping audio
- VAD filtering
- Audio conversion or resampling before provider calls
- Optional post-processing after recognition
- Paste/copy behavior
- History storage

## Privacy Requirements

Provider implementations should not write these values to logs, history, exports, or other machine-readable data:

- API keys
- Provider secrets
- Previous clipboard contents
- Raw provider authorization headers
- Full request payloads that include secrets

Errors should be summarized and redacted before being shown or stored.

## Realtime Recognition

Realtime providers can stream partial text to the overlay while recording is active. Partial text is for preview only unless the provider returns it as a final transcript.

The current public boundary is:

- Final ASR text can enter the normal output chain.
- Partial realtime preview should not be stored as transcript history.
- Stopping recording should converge into the same post-processing and paste path used by non-realtime recognition.

## Adding a Provider

When adding a provider:

1. Define provider id, model id, supported languages, and capability metadata.
2. Keep credentials in settings or environment variables, not in code.
3. Add redaction tests for errors and logs.
4. Verify `auto` language behavior against the provider API.
5. Confirm history stores provider/model/language metadata without secrets.
6. Update [model-sources.md](model-sources.md) or provider documentation if new models are introduced.

## Testing Checklist

- Local model path still works with no network.
- Cloud provider fails clearly when credentials are missing.
- Provider errors do not expose credentials.
- Post-processing works with local and cloud transcripts.
- Paste/copy behavior is the same across providers.
- History records provider metadata and final text source.
