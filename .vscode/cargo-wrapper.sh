#!/bin/bash
# Wrapper that strips --lockfile-path (unsupported in cargo <=1.97)
# and +toolchain syntax from rust-analyzer's cargo calls.
# Portable: uses $HOME to locate the esp toolchain dynamically.

ESP_CARGO="$HOME/.rustup/toolchains/esp/bin/cargo"
export PATH="$HOME/.rustup/toolchains/esp/bin:$PATH"

args=()
skip_next=false
for arg in "$@"; do
    if $skip_next; then
        skip_next=false
        continue
    fi
    if [[ "$arg" == "--lockfile-path" ]]; then
        skip_next=true
        continue
    fi
    if [[ "$arg" == --lockfile-path=* ]]; then
        continue
    fi
    if [[ "$arg" == +* ]]; then
        continue
    fi
    args+=("$arg")
done
exec "$ESP_CARGO" "${args[@]}"
