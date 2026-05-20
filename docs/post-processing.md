# Post-Processing

Post-processing is an optional step after ASR. It takes transcript text and applies a selected preset to produce cleaner or more structured output.

Examples:

- Clean dictation
- Format as Markdown
- Convert a transcript into a concise message
- Preserve technical terms while removing filler words

## Presets

A preset stores the prompt and output behavior for a post-processing task. Built-in presets are examples. Users can copy, edit, create, delete, and test their own presets.

The core behavior is:

```text
raw transcript -> selected preset -> post-processed text -> paste/copy/history
```

Post-processing is separate from ASR. It can be used with either local recognition or cloud recognition.

## Data Model

History keeps three text layers:

| Layer               | Meaning                             |
| ------------------- | ----------------------------------- |
| Raw ASR text        | Direct recognition result           |
| Post-processed text | Output from the selected preset     |
| User-edited text    | Manual correction saved by the user |

When displaying or copying final text, SpeakMore prefers:

```text
user-edited text -> post-processed text -> raw ASR text
```

## Failure Behavior

If post-processing fails, SpeakMore should keep the raw ASR transcript available and show a visible error. A post-processing failure should not discard the original transcript.

Stored error summaries should be redacted. Provider API keys and authorization headers must not enter history.

## Preset Design Rules

Preset prompts should:

- Preserve user meaning unless the preset clearly asks for rewriting
- Avoid adding facts not present in the transcript
- Be inspectable and editable by the user
- Keep provider/model configuration separate from transcript history

Preset prompts should not:

- Include hardcoded private project names by default
- Include API keys or local file paths
- Silently overwrite raw transcripts
- Auto-learn from history without explicit user confirmation

## Testing Checklist

- Built-in presets can be selected and tested.
- A copied preset can be edited without changing the built-in source.
- Failed post-processing falls back to raw ASR text.
- History records preset id/version and run status.
- User-edited text takes precedence over generated text.
