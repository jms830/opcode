#!/usr/bin/env bash
#
# installAppImageWSL2.sh
#
# Usage: ./installAppImageWSL2.sh /full/path/to/AppImageFile
#
set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 /full/path/to/AppImageFile"
    exit 1
fi

APPIMAGE="$1"

# Check if appimage exists
if [ ! -f "$APPIMAGE" ]; then
    echo "Error: file not found: $APPIMAGE"
    exit 1
fi

APPIMAGE_OPT=$(basename -- "$APPIMAGE")
APPIMAGE_DIR=~/.local/bin/"$APPIMAGE_OPT"
# Create standard per-user directories if not already there
mkdir -p $APPIMAGE_DIR
mv "$APPIMAGE" "$APPIMAGE_DIR" && chmod a+x "$APPIMAGE_DIR/$APPIMAGE_OPT"
"$APPIMAGE_DIR/$APPIMAGE_OPT" --appimage-extract

mv squashfs-root/* "$APPIMAGE_DIR/"
rm -rf squashfs-root

# Create launcher
TARGET="$APPIMAGE_DIR/AppRun"

# Create a wrapper that resolves the real path and runs from there
cat > "$HOME/.local/bin/opcode" <<'WRAP'
#!/usr/bin/env bash
set -euo pipefail
TARGET="__TARGET__"
DIR="$(dirname "$(readlink -f "$TARGET")")"
cd "$DIR"
exec "$TARGET" "$@"
WRAP

# Inject your real target path and make executable
sed -i "s|__TARGET__|$TARGET|g" "$HOME/.local/bin/opcode"
chmod +x "$HOME/.local/bin/opcode"

# Check PATH
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
    echo ""
    echo "⚠️  Reminder: add this line to your ~/.bashrc or ~/.zshrc:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
echo "✅ Installation complete."
