const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = path.resolve(__dirname, '..');
const packagesDir = path.join(rootDir, 'packages');

console.log('==> Preparing packages...');
execSync('node scripts/package-all.js', { stdio: 'inherit', cwd: rootDir });

console.log('\n==> Publishing platform binary packages...');
if (fs.existsSync(packagesDir)) {
  const dirs = fs.readdirSync(packagesDir);
  for (const dir of dirs) {
    const pkgPath = path.join(packagesDir, dir);
    const binDir = path.join(pkgPath, 'bin');
    // Only real binaries count; ignore .gitkeep placeholders so empty
    // platform packages (not built on this machine) are not published.
    const binFiles = fs.existsSync(binDir)
      ? fs.readdirSync(binDir).filter((f) => f !== '.gitkeep')
      : [];
    
    // Only publish if binary is present
    if (binFiles.length > 0) {
      console.log(`\nPublishing ${dir}...`);
      try {
        execSync('npm publish --access public', { stdio: 'inherit', cwd: pkgPath });
      } catch (err) {
        console.error(`Failed to publish ${dir}: ${err.message}`);
      }
    } else {
      console.log(`Skipping ${dir} (no binary compiled for this platform yet).`);
    }
  }
}

console.log('\n==> Publishing root bloodhound-search package...');
try {
  execSync('npm publish --access public', { stdio: 'inherit', cwd: rootDir });
  console.log('\n[SUCCESS] Successfully published bloodhound-search to npm!');
} catch (err) {
  console.error(`Failed to publish root package: ${err.message}`);
}
