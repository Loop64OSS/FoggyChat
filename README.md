# FoggyChat

FoggyChat is an anonymous, fully decentralized, end-to-end encrypted direct messaging solution with its own lightweight transfer protocol.

You can host your own server on your device if you seek for total control and privacy.

Built with the Tauri framework (Rust backend), ensuring performance, stability, and a minimal system footprint.

# Compilation

## Install dependencies

-   rust: https://rust-lang.org/tools/install/
-   Node.js: https://nodejs.org/en/download/
-   Tauri prerequisites: https://v2.tauri.app/start/prerequisites/

## For mobile targets, additionally follow

-   Mobile configuration: https://v2.tauri.app/start/prerequisites/#configure-for-mobile-targets
-   Android signing: https://v2.tauri.app/distribute/sign/android/
-   iOS signing: https://v2.tauri.app/distribute/sign/ios/

## Build

1. Clone this repository: `git clone https://github.com/Loop64OSS/FoggyChat.git`
2. Enter the project directory: `cd FoggyChat`
3. Create and fill values in `build.env` from `build.env.template` template in `./scripts/`
4. Compile using scripts: Desktop - `./scripts/build-tauri-desktop-release.sh` Android - `./scripts/build-tauri-android-release.sh`
5. Compiled builds should be located in `./src-tauri/target/release/bundle/`

## ⚠️ Beta

FoggyChat is still in beta. It may contain bugs and breaking changes.
