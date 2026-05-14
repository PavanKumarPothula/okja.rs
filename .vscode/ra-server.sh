#!/bin/bash
# Wrapper for rust-analyzer server that sets up PATH so that
# our cargo wrapper (which strips --lockfile-path) is used.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

export PATH="$SCRIPT_DIR/bin:$HOME/.rustup/toolchains/esp/bin:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export RUSTUP_TOOLCHAIN=esp

# Find the bundled rust-analyzer binary from the VS Code extension
RA_BIN=$(find \
    "$HOME/.vscode-server/extensions/rust-lang.rust-analyzer"*/server \
    "$HOME/.vscode/extensions/rust-lang.rust-analyzer"*/server \
    -name "rust-analyzer" -type f 2>/dev/null | sort -V | tail -1)

if [ -z "$RA_BIN" ]; then
    echo "Could not find rust-analyzer binary" >&2
    exit 1
fi

exec "$RA_BIN" "$@"
