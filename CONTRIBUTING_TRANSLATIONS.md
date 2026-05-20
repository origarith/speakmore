# Contributing Translations to SpeakMore

SpeakMore uses i18next for all user-facing UI text. Translation files live under `src/i18n/locales/`.

## Quick Start

1. Fork the repository.
2. Update or add a locale file.
3. Run the translation consistency check.
4. Open a pull request.

```bash
bun run check:translations
```

## File Structure

```text
src/i18n/
├── languages.ts
└── locales/
    ├── en/translation.json
    ├── zh/translation.json
    ├── zh-TW/translation.json
    └── ...
```

English (`en`) is the source locale. Other locales should keep the same JSON keys and translate only the values.

## Adding a Language

Create a locale folder:

```bash
mkdir src/i18n/locales/<language-code>
cp src/i18n/locales/en/translation.json src/i18n/locales/<language-code>/translation.json
```

Then add metadata in `src/i18n/languages.ts`:

```typescript
export const LANGUAGE_METADATA = {
  en: { name: "English", nativeName: "English", priority: 1 },
  de: { name: "German", nativeName: "Deutsch" },
};
```

For right-to-left languages, include `direction: "rtl"`.

## Translation Rules

Do:

- Translate values, not JSON keys.
- Preserve variables such as `{{error}}`, `{{model}}`, and `{{count}}`.
- Keep UI text concise.
- Use natural wording for your locale.
- Keep product and project names unchanged unless there is an established local form.

Do not:

- Translate JSON keys.
- Remove variables.
- Add secrets, local paths, or test credentials.
- Change formatting in a way that breaks JSON parsing.

Example:

```json
{
  "downloadModel": "Failed to download model: {{error}}"
}
```

Correct French translation:

```json
{
  "downloadModel": "Echec du telechargement du modele : {{error}}"
}
```

Incorrect translation:

```json
{
  "telechargerModele": "Echec du telechargement du modele : {{erreur}}"
}
```

## Supported Locales

Current locale folders:

| Language            | Code    |
| ------------------- | ------- |
| Arabic              | `ar`    |
| Bulgarian           | `bg`    |
| Czech               | `cs`    |
| German              | `de`    |
| English             | `en`    |
| Spanish             | `es`    |
| French              | `fr`    |
| Hebrew              | `he`    |
| Italian             | `it`    |
| Japanese            | `ja`    |
| Korean              | `ko`    |
| Polish              | `pl`    |
| Portuguese          | `pt`    |
| Russian             | `ru`    |
| Swedish             | `sv`    |
| Turkish             | `tr`    |
| Ukrainian           | `uk`    |
| Vietnamese          | `vi`    |
| Simplified Chinese  | `zh`    |
| Traditional Chinese | `zh-TW` |

## Testing

Run:

```bash
bun run check:translations
bun run lint
bun run build
```

Manual checks are useful for languages with long strings or right-to-left layout. Open the app, switch the UI language in settings, and check onboarding, settings, model management, and post-processing screens.
