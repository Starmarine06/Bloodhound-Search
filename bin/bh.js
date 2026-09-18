#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

function getPlatformPackage() {
  const platform = process.platform;
  const arch = process.arch;

  const platformMap = {
    win32: {
      x64: '@dev_nambiar/bloodhound-win32-x64',
      arm64: '@dev_nambiar/bloodhound-win32-arm64',
    },
    darwin: {
      x64: '@dev_nambiar/bloodhound-darwin-x64',
      arm64: '@dev_nambiar/bloodhound-darwin-arm64',
    },
    linux: {
      x64: '@dev_nambiar/bloodhound-linux-x64',
      arm64: '@dev_nambiar/bloodhound-linux-arm64',
    },
  };

  return platformMap[platform]?.[arch];
}

function getBinaryName() {
  return process.platform === 'win32' ? 'bh.exe' : 'bh';
}

function resolveBinary() {
  const binName = getBinaryName();
  const pkgName = getPlatformPackage();

  // 1. Try resolving from installed platform npm package
  if (pkgName) {
    try {
      const pkgPath = require.resolve(`${pkgName}/package.json`);
      const binPath = path.join(path.dirname(pkgPath), 'bin', binName);
      if (fs.existsSync(binPath)) {
        return binPath;
      }
    } catch {
      // Platform package not found via require.resolve, check other locations
    }
  }

  // 2. Try local target release / debug binary (for local development)
  const localTargets = [
    path.resolve(__dirname, '..', 'target', 'release', binName),
    path.resolve(__dirname, '..', 'target', 'debug', binName),
    path.resolve(__dirname, binName),
  ];
  for (const p of localTargets) {
    if (fs.existsSync(p)) return p;
  }

  // 3. Try sibling platform packages in node_modules or packages folder
  if (pkgName) {
    const candidatePaths = [
      path.resolve(__dirname, '..', 'node_modules', pkgName, 'bin', binName),
      path.resolve(__dirname, '..', '..', pkgName, 'bin', binName),
      path.resolve(__dirname, '..', 'packages', pkgName.replace(/^@[^/]+\/bloodhound-/, '').replace(/^@[^/]+\//, ''), 'bin', binName),
    ];
    for (const p of candidatePaths) {
      if (fs.existsSync(p)) return p;
    }
  }

  return null;
}

function main() {
  const binaryPath = resolveBinary();

  if (!binaryPath) {
    const pkgName = getPlatformPackage();
    console.error(`\x1b[31merror:\x1b[0m Could not find native Bloodhound binary for platform '${process.platform}' and arch '${process.arch}'.`);
    if (pkgName) {
      console.error(`Please ensure '${pkgName}' is installed or build locally with 'cargo build --release'.`);
    } else {
      console.error(`Your platform (${process.platform}-${process.arch}) is not currently supported with pre-built binaries.`);
    }
    process.exit(1);
  }

  const result = spawnSync(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: true,
  });

  if (result.error) {
    console.error(`\x1b[31merror:\x1b[0m Failed to execute native binary: ${result.error.message}`);
    process.exit(1);
  }

  if (result.status !== null) {
    process.exit(result.status);
  } else if (result.signal) {
    process.kill(process.pid, result.signal);
  } else {
    process.exit(0);
  }
}

main();
