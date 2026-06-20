#!/usr/bin/env node

const { spawn, execSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const https = require('https');
const http = require('http');

const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
const version = pkg.version;
const repo = 'sonyarianto/santui';
const binaryName = process.platform === 'win32' ? 'santui.exe' : 'santui';
const binaryPath = path.join(__dirname, binaryName);

// ── Download helpers ──

function getTarget() {
  const os = process.platform;
  const arch = process.arch;
  if (arch !== 'x64' && arch !== 'arm64') die(`Unsupported architecture: ${arch}`);
  if (os === 'win32') return 'x86_64-pc-windows-msvc';
  if (os === 'darwin') {
    if (arch === 'arm64') return 'aarch64-apple-darwin';
    die('Intel Mac (x64) is not supported yet. Build from source instead:\n  git clone https://github.com/sonyarianto/santui.git && cd santui && cargo build --workspace');
  }
  if (os === 'linux') return 'x86_64-unknown-linux-gnu';
  die(`Unsupported platform: ${os}`);
}

function getArchiveExt() { return process.platform === 'win32' ? 'zip' : 'tar.gz'; }

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    const protocol = url.startsWith('https') ? https : http;
    protocol.get(url, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        file.close(); fs.unlinkSync(dest);
        return download(res.headers.location, dest).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) { file.close(); fs.unlinkSync(dest); return reject(new Error(`HTTP ${res.statusCode}`)); }
      res.pipe(file);
      file.on('finish', () => { file.close(); resolve(); });
    }).on('error', (err) => { file.close(); fs.unlinkSync(dest, () => {}); reject(err); });
  });
}

function die(msg) { console.error(msg); process.exit(1); }

async function downloadBinary() {
  const target = getTarget();
  const ext = getArchiveExt();
  const archiveUrl = `https://github.com/${repo}/releases/download/v${version}/santui-${target}.${ext}`;

  console.error(`Downloading Santui v${version} (${target})...`);
  console.error(`  ${archiveUrl}`);

  const tmpDir = fs.mkdtempSync(path.join(__dirname, 'tmp-'));
  const archivePath = path.join(tmpDir, `santui.${ext}`);

  try {
    await download(archiveUrl, archivePath);
    console.error('  Extracting...');

    if (ext === 'zip') {
      execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${tmpDir}' -Force"`, { stdio: 'pipe' });
      const files = fs.readdirSync(tmpDir);
      const exeFile = files.find(f => f.endsWith('.exe'));
      if (!exeFile) throw new Error('santui.exe not found in archive');
      fs.copyFileSync(path.join(tmpDir, exeFile), binaryPath);
    } else {
      execSync(`tar xzf '${archivePath}' -C '${tmpDir}'`, { stdio: 'pipe' });
      const extracted = path.join(tmpDir, binaryName);
      if (fs.existsSync(extracted)) {
        fs.copyFileSync(extracted, binaryPath);
      } else {
        const items = fs.readdirSync(tmpDir);
        let found = false;
        for (const item of items) {
          const itemPath = path.join(tmpDir, item);
          if (fs.statSync(itemPath).isDirectory()) {
            const sub = path.join(itemPath, binaryName);
            if (fs.existsSync(sub)) { fs.copyFileSync(sub, binaryPath); found = true; break; }
          }
        }
        if (!found) throw new Error(`${binaryName} not found in archive`);
      }
    }

    if (process.platform !== 'win32') execSync(`chmod +x '${binaryPath}'`, { stdio: 'pipe' });
    console.error('  [OK] Santui binary downloaded');
  } catch (err) {
    die(`Failed to download Santui: ${err.message}`);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

// ── Main ──

(async () => {
  // Download binary on first run if missing
  if (!fs.existsSync(binaryPath)) {
    await downloadBinary();
  }

  // Spawn the binary with all passed arguments
  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: false,
  });

  child.on('close', (code) => process.exit(code));
  child.on('error', (err) => die(`Failed to start Santui: ${err.message}`));
})();
