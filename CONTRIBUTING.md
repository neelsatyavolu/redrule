# Contributing to Redrule

Thanks for helping. Bug reports, fixes and small focused features are all welcome. For anything large, open an issue first so we can agree on the approach.

## Build from source

You need:

- A Mac with Apple Silicon running macOS 15 or later
- Xcode or the Xcode Command Line Tools (`xcode-select --install`)
- Rust 1.88 or later (`rustup`), the version in `src-tauri/Cargo.toml`
- CMake (`brew install cmake`), used to build llama.cpp
- Node.js 22 or later and pnpm 10

Then:

```bash
pnpm install
pnpm tauri dev          # run the app with hot reload
```

The first build compiles llama.cpp and sherpa-onnx and takes a while. On first launch the app downloads its speech model.

To design the interface without the Rust backend, run `pnpm dev` and open `/preview.html?view=notes`. It renders the UI with sample data.

## Tests

```bash
cd src-tauri && cargo test --workspace && cd .. && pnpm test
```

The sharing service in `sharing/` has its own tests:

```bash
cd sharing && npm ci && npm test && npm run build
```

## Signing and permissions

Build and install a local copy with ad-hoc signing:

```bash
scripts/install-desktop.sh --adhoc --open
```

`--adhoc` needs no certificates. The catch: macOS ties Microphone and Screen & System Audio Recording permissions, and Keychain "Always Allow", to the app's signature. An ad-hoc signature changes with every build, so macOS asks again after each one. Quit and reopen the app after granting Screen & System Audio Recording.

Without `--adhoc`, the script signs with the maintainer's Developer ID, which contributors don't have.

## Code style

- Pure logic goes in `src-tauri/crates/core` with unit tests. Code that touches macOS, audio or the network goes in `crates/engine` or `src-tauri/src`.
- Keep functions small and files focused. Prefer returning new values over mutating shared state.
- Comments explain why, not what.
- Errors shown to people are plain sentences that say what happened, e.g. "The Application Support folder was not found."
- The frontend is React with strict TypeScript, Tailwind and zustand. Calls to Tauri commands go through `src/lib/api.ts`.
- The product is called Redrule in anything a person sees. Older internal names such as `minutes`, `minutes_lib`, `MinutesCore`, the `Minutes` Keychain service and `co.nenu.minutes` stay as they are: they keep existing installs and data working.
- Keep copy short and plain.

## Pull request checklist

- [ ] One focused change per pull request, with a clear description of what and why
- [ ] Tests added or updated, and `cargo test --workspace` and `pnpm test` pass
- [ ] `pnpm build` passes (it type-checks the frontend)
- [ ] Sharing tests pass if you changed `sharing/`
- [ ] Tried in the running app for anything a person can see or hear
- [ ] No secrets, personal paths or real meeting content in code, tests or screenshots
- [ ] Privacy policy (`website/privacy.html`) updated if the change sends new data off the Mac

By contributing, you agree that your contribution is licensed under the MIT License.
