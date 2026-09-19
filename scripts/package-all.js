const fs = require('fs');
const path = require('path');

const VERSION = require('../package.json').version;

const TARGETS = [
  {
    name: 'win32-x64',
    os: 'win32',
    cpu: 'x64',
    rustTarget: 'x86_64-pc-windows-msvc',
    binName: 'bh.exe'
  },
  {
    name: 'win32-arm64',
    os: 'win32',
    cpu: 'arm64',
    rustTarget: 'aarch64-pc-windows-msvc',
    binName: 'bh.exe'
  },
  {
    name: 'darwin-x64',
    os: 'darwin',
    cpu: 'x64',
    rustTarget: 'x86_64-apple-darwin',
    binName: 'bh'
  },
  {
    name: 'darwin-arm64',
    os: 'darwin',
    cpu: 'arm64',
    rustTarget: 'aarch64-apple-darwin',
    binName: 'bh'
  },
  {
    name: 'linux-x64',
    os: 'linux',
    cpu: 'x64',
    rustTarget: 'x86_64-unknown-linux-musl',
    binName: 'bh'
  },
  {
    name: 'linux-arm64',
    os: 'linux',
    cpu: 'arm64',
    rustTarget: 'aarch64-unknown-linux-musl',
    binName: 'bh'
  }
];

const rootDir = path.resolve(__dirname, '..');
const packagesDir = path.join(rootDir, 'packages');

if (!fs.existsSync(packagesDir)) {
  fs.mkdirSync(packagesDir, { recursive: true });
}

for (const target of TARGETS) {
  const pkgDir = path.join(packagesDir, target.name);
  const binDir = path.join(pkgDir, 'bin');

  fs.mkdirSync(binDir, { recursive: true });

  const rootPkg = require('../package.json');

  const pkgJson = {
    name: `@dev_nambiar/bloodhound-${target.name}`,
    version: VERSION,
    description: `Native pre-built binary of Bloodhound for ${target.os}-${target.cpu}`,
    os: [target.os],
    cpu: [target.cpu],
    files: ['bin'],
    license: rootPkg.license || 'MIT',
    repository: rootPkg.repository,
    bugs: rootPkg.bugs,
    homepage: rootPkg.homepage,
    publishConfig: {
      access: "public"
    }
  };

  fs.writeFileSync(
    path.join(pkgDir, 'package.json'),
    JSON.stringify(pkgJson, null, 2) + '\n'
  );

  // If local target binary exists for current system, copy it into package bin folder
  const localReleaseBin = path.join(rootDir, 'target', target.rustTarget, 'release', target.binName);
  const defaultReleaseBin = path.join(rootDir, 'target', 'release', target.binName);

  let sourceBin = null;
  if (fs.existsSync(localReleaseBin)) {
    sourceBin = localReleaseBin;
  } else if (process.platform === target.os && (process.arch === target.cpu || (target.cpu === 'x64' && process.arch === 'x64')) && fs.existsSync(defaultReleaseBin)) {
    sourceBin = defaultReleaseBin;
  }

  if (sourceBin) {
    const destBin = path.join(binDir, target.binName);
    fs.copyFileSync(sourceBin, destBin);
    if (process.platform !== 'win32') {
      fs.chmodSync(destBin, 0o755);
    }
    console.log(`[OK] Populated @dev_nambiar/bloodhound-${target.name} with binary.`);
  } else {
    console.log(`[INFO] Created package structure for @dev_nambiar/bloodhound-${target.name}.`);
  }
}

console.log('\nPackage preparation complete.');
