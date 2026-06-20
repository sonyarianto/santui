const fs = require('fs');
const path = '.github/workflows/release.yml';
let content = fs.readFileSync(path, 'utf8');

// Fix 1: Replace v\\$version (which YAML sees as unknown escape) 
// with a bash variable indirection approach
// Old: "v\\$version/santui-..."  (this confuses YAML parsers)
// New: use DOLLAR_VAR variable in bash to avoid the escape

// Replace the problematic line with a version that uses a variable
const oldStr = `            "url": "https://github.com/sonyarianto/santui/releases/download/v\\$version/santui-x86_64-pc-windows-msvc.zip"`;
const newStr = `            "url": "https://github.com/sonyarianto/santui/releases/download/v${DOLLAR_SIGN}version/santui-x86_64-pc-windows-msvc.zip"`;

// Actually let me just use a different approach
// Replace the literal backslash-dollar with just using the bash variable approach
const oldText = `"url": "https://github.com/sonyarianto/santui/releases/download/v\\$version/santui-x86_64-pc-windows-msvc.zip"`;
const newText = `"url": "https://github.com/sonyarianto/santui/releases/download/v\\${DOLLAR}version/santui-x86_64-pc-windows-msvc.zip"`;

// OK the JS escaping is a nightmare. Let me just find and replace literally.
// Read the file as buffer to avoid encoding issues
const buf = fs.readFileSync(path);
const idx = buf.indexOf('v\\$version', 'utf8');
// That won't work either. Let me try yet another approach.

// Just use String.replace with a regex
const result = content.replace(/v\\\$version/g, 'vDOLLARSIGNversion');
fs.writeFileSync(path, content);
console.log('Done. Replaced count:', (content.match(/v\\\$version/g) || []).length);
