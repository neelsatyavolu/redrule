# Security

## Reporting a vulnerability

Please report vulnerabilities privately. Don't open a public issue.

- Preferred: GitHub private vulnerability reporting. On this repository, open **Security**, then **Report a vulnerability**.
- Or email **neel@xanom.co**

Include what you found, how to reproduce it, and the Redrule version. We'll reply as soon as we can. Please give us a reasonable time to fix the issue before you disclose it.

## Scope

In scope:

- The Redrule Mac app in `src-tauri/` and `src/`
- The sharing service in `sharing/` and the shared pages at `redrule.n3el.dev/s/` and `/f/`
- The update feed and how the app verifies updates
- How the app stores tokens and API keys, and what it sends to note-writing providers
- The daily usage ping (`src-tauri/src/usage_ping.rs`) and what it sends

Out of scope:

- ChatGPT, Grok, OpenAI, Anthropic, Google, Vercel, GitHub, Hugging Face and Sentry themselves. Report those to the provider.
- The legacy Swift app in `Sources/`, which is no longer released
- Attacks that need an already compromised Mac or physical access to an unlocked one
- Denial of service by sending large volumes of traffic

## Supported versions

Only the latest release gets fixes. The app updates itself, so please check that an issue still happens on the latest version.
