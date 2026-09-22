# Minutes agent notes

Codex/Grok provider settings and the live model list are maintained in [shared-ai-auth](https://github.com/neelsatyavolu/shared-ai-auth). The Swift and Rust apps keep their native localhost callback listeners and secure token storage. Both refresh `models.json` from the public repository and use a bundled fallback offline. Add IDs to a local blacklist only when Minutes should hide them; new catalog entries show by default. Keep Swift and Rust model handling aligned when changing either app.
