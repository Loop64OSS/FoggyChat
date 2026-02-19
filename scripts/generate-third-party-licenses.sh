npx generate-license-file --input package.json --output licenses/third-party-licenses-npm.txt
cd src-tauri
cargo bundle-licenses --output ../licenses/third-party-licenses-cargo.txt