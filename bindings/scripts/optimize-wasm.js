const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const wasmPath = path.join(__dirname, '../target/wasm32-unknown-unknown/release/trustlink_escrow.wasm');

if (!fs.existsSync(wasmPath)) {
    console.error('WASM file not found:', wasmPath);
    process.exit(1);
}

const statsBefore = fs.statSync(wasmPath);
const sizeBefore = statsBefore.size;

try {
    execSync(`wasm-opt -Oz --strip-debug --vacuum "${wasmPath}" -o "${wasmPath}"`, { stdio: 'inherit' });
    const statsAfter = fs.statSync(wasmPath);
    const sizeAfter = statsAfter.size;
    console.log(`WASM optimized: ${sizeBefore} bytes -> ${sizeAfter} bytes (${((sizeBefore - sizeAfter) / sizeBefore * 100).toFixed(1)}% reduction)`);
} catch (error) {
    console.error('wasm-opt not found. Install binaryen:');
    console.error('  macOS: brew install binaryen');
    console.error('  Linux: apt install binaryen');
    process.exit(1);
}