#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

// Find the downloaded binary
const binaryName = process.platform === 'win32' ? 'santui.exe' : 'santui';
const binaryPath = path.join(__dirname, binaryName);

// Fallback: check node_modules/.bin
const fallbackPath = path.join(__dirname, '..', '.bin', binaryName);

const exe = fs.existsSync(binaryPath) ? binaryPath : fallbackPath;

if (!fs.existsSync(exe)) {
  console.error('Santui binary not found. Run `npm install` again or check your installation.');
  process.exit(1);
}

// Spawn the binary with all passed arguments
const child = spawn(exe, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

child.on('close', (code) => {
  process.exit(code);
});

child.on('error', (err) => {
  console.error('Failed to start Santui:', err.message);
  process.exit(1);
});
