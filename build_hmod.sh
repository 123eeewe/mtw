#!/usr/bin/env bash
# Сборка .hmod-пакета MouseTweaks-RS для Horizon.
# usage: ./build_hmod.sh [aarch64-linux-android]
set -euo pipefail
cd "$(dirname "$0")"

# По умолчанию — целевая платформа Horizon (Windows x64).
TARGET="${1:-x86_64-pc-windows-gnu}"
cargo build --release --target "$TARGET"
case "$TARGET" in
  *windows*) NATIVE_NAME="mousetweaks_rs.dll" ;;
  *apple*)   NATIVE_NAME="libmousetweaks_rs.dylib" ;;
  *)         NATIVE_NAME="libmousetweaks_rs.so" ;;
esac
SO="target/$TARGET/release/$NATIVE_NAME"
[[ -f "$SO" ]] || { echo "native lib not found: $SO" >&2; exit 1; }

NAME="MouseTweaks-RS_v$(grep -m1 '^version' Cargo.toml | tr -d '"' | awk '{print $3}').hmod"
STAGE="build_hmod/mousetweaks_rs_v1_0_0"

mkdir -p "$STAGE/pe_packs/render_containers/mtr_native" \
         "$STAGE/resource_packs/mtr_assets/textures" \
         "$STAGE/config"
rm -f "$STAGE"/pe_packs/render_containers/mtr_native/libmousetweaks_rs.* \
      "$STAGE"/pe_packs/render_containers/mtr_native/mousetweaks_rs.*
cp "$SO" "$STAGE/pe_packs/render_containers/mtr_native/"
cp config/mousetweaks_rs.toml "$STAGE/config/"
# Обновляем путь нативной точки входа в pack_icon.json под текущую платформу
python3 - "$STAGE/pack_icon.json" "pe_packs/render_containers/mtr_native/$NATIVE_NAME" <<'EOF'
import json, sys
p, native = sys.argv[1], sys.argv[2]
d = json.load(open(p))
d["entry_native"] = native
json.dump(d, open(p, "w"), ensure_ascii=False, indent=2)
EOF
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
