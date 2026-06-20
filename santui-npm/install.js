const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');
const { createWriteStream, readFileSync, writeFileSync } = require('fs');
const { execSync } = require('child_process');

const pkg = JSON.parse(readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
const version = pkg.version;
const repo = 'sonyarianto/santui';

// Map OS + arch → Rust target triple
function getTarget() {
  const os = process.platform;
  const arch = process.arch;

  if (arch !== 'x64' && arch !== 'arm64') {
    console.error(`Unsupported architecture: ${arch}`);
    process.exit(1);
  }

  if (os === 'win32') {
    return 'x86_64-pc-windows-msvc';
  }
  if (os === 'darwin') {
    if (arch === 'arm64') return 'aarch64-apple-darwin';
    console.error('Intel Mac (x64) is not supported yet. Build from source instead:');
    console.error('  git clone https://github.com/sonyarianto/santui.git && cd santui && cargo build --workspace');
    process.exit(1);
  }
  if (os === 'linux') {
    return 'x86_64-unknown-linux-gnu';
  }

  console.error(`Unsupported platform: ${os}`);
  process.exit(1);
}

function getArchiveExt() {
  return process.platform === 'win32' ? 'zip' : 'tar.gz';
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    const protocol = url.startsWith('https') ? https : http;

    protocol.get(url, (response) => {
      // Handle redirects (GitHub)
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        file.close();
        fs.unlinkSync(dest);
        return download(response.headers.location, dest).then(resolve).catch(reject);
      }

      if (response.statusCode !== 200) {
        file.close();
        fs.unlinkSync(dest);
        return reject(new Error(`HTTP ${response.statusCode}: ${response.statusMessage}`));
      }

      response.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    }).on('error', (err) => {
      file.close();
      fs.unlinkSync(dest, () => {});
      reject(err);
    });
  });
}

async function install() {
  const target = getTarget();
  const ext = getArchiveExt();
  const archiveUrl = `https://github.com/${repo}/releases/download/v${version}/santui-${target}.${ext}`;
  const binaryName = process.platform === 'win32' ? 'santui.exe' : 'santui';
  const destPath = path.join(__dirname, binaryName);

  console.log(`Downloading Santui v${version} (${target})...`);
  console.log(`  ${archiveUrl}`);

  const tmpDir = fs.mkdtempSync(path.join(__dirname, 'tmp-'));
  const archivePath = path.join(tmpDir, `santui.${ext}`);

  try {
    // Download archive
    await download(archiveUrl, archivePath);

    // Extract
    console.log('  Extracting...');
    if (ext === 'zip') {
      execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${tmpDir}' -Force"`, { stdio: 'pipe' });
      // Find the exe in the extracted folder
      const files = fs.readdirSync(tmpDir);
      const exeFile = files.find(f => f.endsWith('.exe'));
      if (!exeFile) throw new Error('santui.exe not found in archive');
      fs.copyFileSync(path.join(tmpDir, exeFile), destPath);
    } else {
      execSync(`tar xzf '${archivePath}' -C '${tmpDir}'`, { stdio: 'pipe' });
      // Binary is at root of archive (tar czf ... -C staging .)
      const extractedBinary = path.join(tmpDir, binaryName);
      if (fs.existsSync(extractedBinary)) {
        fs.copyFileSync(extractedBinary, destPath);
      } else {
        // Fallback: check subdirectory (staging/)
        const items = fs.readdirSync(tmpDir);
        let found = false;
        for (const item of items) {
          const itemPath = path.join(tmpDir, item);
          if (fs.statSync(itemPath).isDirectory()) {
            const subBinary = path.join(itemPath, binaryName);
            if (fs.existsSync(subBinary)) {
              fs.copyFileSync(subBinary, destPath);
              found = true;
              break;
            }
          }
        }
        if (!found) throw new Error(`${binaryName} not found in archive`);
      }
    }

    // Make executable on Unix
    if (process.platform !== 'win32') {
      execSync(`chmod +x '${destPath}'`, { stdio: 'pipe' });
    }

    console.log('  [OK] Binary installed');
  } catch (err) {
    console.error(`  [FAIL] ${err.message}`);
    process.exit(1);
  } finally {
    // Cleanup temp
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

install();
