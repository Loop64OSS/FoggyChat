# FoggyChat

FoggyChat is an anonymous, fully decentralized, end-to-end encrypted direct messaging solution with its own lightweight transfer protocol.

You can host your own server on your device if you seek for total control and privacy.

Built with the Tauri framework (Rust backend), ensuring performance, stability, and a minimal system footprint.

# Compilation

## Install dependencies

- Rust: https://rust-lang.org/tools/install/
- Node.js: https://nodejs.org/en/download/
- Tauri prerequisites: https://v2.tauri.app/start/prerequisites/ (Also install tauri-cli using `cargo install tauri-cli --version "^2.0.0" --locked`)

## For mobile targets, additionally follow

- Mobile configuration: https://v2.tauri.app/start/prerequisites/#configure-for-mobile-targets
- Android signing: https://v2.tauri.app/distribute/sign/android/
- iOS signing: https://v2.tauri.app/distribute/sign/ios/

\* On mobile platforms, applications must be digitally signed in order to be installable.

## Build

1. Clone this repository: `git clone https://github.com/Loop64OSS/FoggyChat.git`
2. Enter the project directory: `cd FoggyChat`
3. Install dependencies: `cd src && npm install && cd ..`
4. Create and fill values in `./scripts/build.env` from `./scripts/build.env.template`
5. Compile using scripts: (SH) Desktop - `./scripts/build-tauri-desktop-release.sh` Android - `./scripts/build-tauri-android-release.sh`, alternatively use `cargo tauri build --verbose` for desktop
6. Compiled builds should be located in `./src-tauri/target/release/bundle/`

\* This release includes third-party dependencies. See licenses/third-party-licenses-npm.txt and licenses/third-party-licenses-cargo.txt for full license texts.

## ⚠️ Beta

FoggyChat is still in beta. It may contain bugs and breaking changes.
