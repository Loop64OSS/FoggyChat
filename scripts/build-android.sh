cd ~/FoggyChat/client-gui
cargo tauri android build --debug
~/Android/Sdk/platform-tools/adb -s RFCX60K0DQX install ~/FoggyChat/client-gui/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk