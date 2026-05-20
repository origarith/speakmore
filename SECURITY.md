# Security Policy

## Reporting a Vulnerability

Please do not report security vulnerabilities in public issues.

Use GitHub's private vulnerability reporting flow if it is available for this repository:

```text
https://github.com/OrigArith/SpeakMore/security/advisories/new
```

If private vulnerability reporting is unavailable, open a minimal public issue that says you need a private security contact. Do not include exploit details, secrets, transcripts, provider credentials, or private logs in that issue.

## Scope

Security-sensitive areas include:

- Provider API key storage and redaction
- Cloud ASR request handling
- Transcript history and export behavior
- Clipboard and paste handling
- Global shortcut and accessibility integrations
- Release signing and updater metadata

## Supported Versions

SpeakMore is preparing for its first public source release. Until public releases exist, security fixes apply to the `main` branch.

## Handling Sensitive Data

Bug reports and PRs should not include:

- Provider API keys
- Authorization headers
- Private transcripts
- Audio recordings with sensitive content
- Full local history databases
- Logs that have not been checked for secrets

When logs are needed, redact secrets and reduce them to the smallest useful reproduction.
