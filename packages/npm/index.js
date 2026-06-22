#!/usr/bin/env node

const { spawn, execSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const https = require('https');
const http = require('http');

const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
const version = pkg.version;
const repo = 'sonyarianto/santui';
const isWin = process.platform === 'win32';
const binaryName = isWin ? 'santui.exe' : 'santui';
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
    } else {
      execSync(`tar xzf '${archivePath}' -C '${tmpDir}'`, { stdio: 'pipe' });
    }

    // Determine extracted root (some archives wrap in a top-level folder)
    const entries = fs.readdirSync(tmpDir).filter(e => e !== path.basename(archivePath));
    let extractedRoot = tmpDir;
    if (entries.length === 1 && fs.statSync(path.join(tmpDir, entries[0])).isDirectory()) {
      extractedRoot = path.join(tmpDir, entries[0]);
    }

    // Validate main binary exists
    if (!fs.existsSync(path.join(extractedRoot, binaryName))) {
      throw new Error(`${binaryName} not found in archive`);
    }

    // Copy all extracted files (binaries + native deps) to package directory
    for (const item of fs.readdirSync(extractedRoot)) {
      const src = path.join(extractedRoot, item);
      const dest = path.join(__dirname, item);
      const stat = fs.statSync(src);
      if (stat.isDirectory()) {
        if (fs.existsSync(dest)) fs.rmSync(dest, { recursive: true, force: true });
        fs.mkdirSync(dest, { recursive: true });
        for (const f of fs.readdirSync(src)) {
          fs.copyFileSync(path.join(src, f), path.join(dest, f));
        }
      } else {
        fs.copyFileSync(src, dest);
        if (!isWin) fs.chmodSync(dest, 0o755);
      }
    }

    if (!isWin) {
      // Ensure all plugin binaries are executable
      for (const bin of fs.readdirSync(__dirname)) {
        if (bin.startsWith('santui') && !bin.endsWith('.js') && !bin.endsWith('.json') && !bin.endsWith('.md')) {
          fs.chmodSync(path.join(__dirname, bin), 0o755);
        }
      }
    }

    console.error('');
    console.error('  ✅ Santui v' + version + ' ready!');
    console.error('  Type "santui" to launch your terminal home base.');
    console.error('');
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
