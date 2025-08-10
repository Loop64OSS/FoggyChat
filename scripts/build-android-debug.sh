#!/bin/bash
. ./scripts/build.env
cd $PROJECT_PATH/client-gui
cargo tauri android build --debug --verbose
$ANDROID_HOME/platform-tools/adb -s $ADB_DEBUG_DEVICE_ID install $PROJECT_PATH/client-gui/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk