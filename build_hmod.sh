#!/usr/bin/env bash
# Сборка .hmod-пакета MouseTweaks-RS для Horizon.
# usage: ./build_hmod.sh [aarch64-linux-android]
set -euo pipefail
cd "$(dirname "$0")"

TARGET="${1:-}"
if [[ -n "$TARGET" ]]; then
  cargo build --release --target "$TARGET"
  SO="target/$TARGET/release/libmousetweaks_rs.so"
else
  cargo build --release
  SO="target/release/libmousetweaks_rs.so"
fi

NAME="MouseTweaks-RS_v$(grep -m1 '^version' Cargo.toml | tr -d '"' | awk '{print $3}').hmod"
STAGE="build_hmod/mousetweaks_rs_v1_0_0"

mkdir -p "$STAGE/pe_packs/render_containers/mtr_native" \
         "$STAGE/resource_packs/mtr_assets/textures" \
         "$STAGE/config"
cp "$SO" "$STAGE/pe_packs/render_containers/mtr_native/"
cp config/mousetweaks_rs.toml "$STAGE/config/"
# manifest.json, pack_icon.json и pack_icon.png лежат в STAGE (или генерируются здесь)

python3 - "$STAGE" "$NAME" <<'EOF'
import zipfile, os, sys
src, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(src):
        for f in sorted(files):
            p = os.path.join(root, f)
            z.write(p, os.path.relpath(p, src))
print("built:", out)
EOF
