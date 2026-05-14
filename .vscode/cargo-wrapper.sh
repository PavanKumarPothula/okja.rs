#!/bin/bash
# Wrapper that strips --lockfile-path (unsupported in cargo 1.95)
# and +toolchain syntax (not understood by cargo directly) from
# rust-analyzer's cargo calls.

# Ensure rustc and other tools are findable
export PATH="/home/pavankup/.rustup/toolchains/esp/bin:$PATH"

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
    # Strip +toolchain args (rustup feature, not cargo)
    if [[ "$arg" == +* ]]; then
        continue
    fi
    args+=("$arg")
done
exec /home/pavankup/.rustup/toolchains/esp/bin/cargo "${args[@]}"
