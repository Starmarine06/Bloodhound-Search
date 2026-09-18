const fs = require('fs');
const path = require('path');

const rootDir = path.resolve(__dirname, '..');
const binName = process.platform === 'win32' ? 'bh.exe' : 'bh';
const srcBin = path.join(rootDir, 'target', 'release', binName);
const dstBin = path.join(rootDir, 'bin', binName);

if (fs.existsSync(srcBin)) {
  fs.copyFileSync(srcBin, dstBin);
  if (process.platform !== 'win32') {
    fs.chmodSync(dstBin, 0o755);
  }
  console.log(`[OK] Copied native binary to bin/${binName}`);
} else {
  console.warn(`[WARN] Release binary not found at ${srcBin}. Run 'cargo build --release' first.`);
}
