#!/bin/bash
. ./scripts/build.env
NO_STRIP=true cargo tauri build --verbose