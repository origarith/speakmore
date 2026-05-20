# History and Privacy

SpeakMore stores local transcript history so users can review, copy, retry, and correct previous transcriptions.

## History Goals

The history system should:

- Preserve the final user-visible transcript
- Keep enough metadata to debug provider behavior
- Distinguish raw ASR text, post-processed text, and user-edited text
- Avoid storing credentials or unrelated clipboard contents
- Support deletion and retention cleanup without orphan records

## Text Priority

History entries can contain:

| Field                 | Meaning                            |
| --------------------- | ---------------------------------- |
| `transcription_text`  | Raw ASR transcript                 |
| `post_processed_text` | Optional post-processing output    |
| `user_edited_text`    | User correction saved from history |

Final text is computed as:

```text
user_edited_text ?? post_processed_text ?? transcription_text
```

## Evidence Tables

The backend records:

- `transcription_history`: current user-visible entry state
- `transcription_runs`: ASR attempts and final results
- `post_process_runs`: post-processing attempts and prompt snapshots
- `history_events`: low-frequency user actions and output events

Realtime partial text should not be stored as transcript history. Only final text should enter the output chain.

## Sensitive Data Boundary

History should not store:

- API keys
- Provider secrets
- Authorization headers
- Previous clipboard contents
- Audio file contents inside JSON event payloads

Stored event payloads should contain only necessary metadata such as status, text length, provider id, model id, or redacted error summaries.

## Testing Checklist

- Deleting an entry removes child runs, events, and audio artifacts.
- Retention cleanup does not leave orphan records.
- Retry creates new run evidence without overwriting old runs.
- User edits affect final text priority.
- List views do not expose full transcript text where summaries are sufficient.
