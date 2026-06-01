#!/bin/bash
set -e

CONTRACT_NAME="trustlink-escrow"
WASM_DIR="target/wasm32-unknown-unknown/release"
WASM_FILE="${WASM_DIR}/${CONTRACT_NAME}.wasm"

echo "Building WASM for ${CONTRACT_NAME}..."
cargo build --target wasm32-unknown-unknown --release

if command -v wasm-opt &> /dev/null; then
    echo "Running wasm-opt optimization..."
    OPTIMIZED_SIZE=$(stat -f%z "${WASM_FILE}" 2>/dev/null || stat -c%s "${WASM_FILE}" 2>/dev/null)
    wasm-opt -Oz --strip-debug --vacuum "${WASM_FILE}" -o "${WASM_FILE}"
    OPTIMIZED_SIZE_AFTER=$(stat -f%z "${WASM_FILE}" 2>/dev/null || stat -c%s "${WASM_FILE}" 2>/dev/null)
    echo "WASM optimized: ${OPTIMIZED_SIZE} bytes -> ${OPTIMIZED_SIZE_AFTER} bytes"
else
    echo "Warning: wasm-opt not found. Install binaryen for WASM optimization."
    echo "Install with: brew install binaryen (macOS) or apt install binaryen (Linux)"
fi

echo "Build complete: ${WASM_FILE}"