#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const os = require("os");

const platform = os.platform();
const arch = os.arch();

const binaries = {
  "linux-x64": "doom-mcp-linux-x64",
  "linux-arm64": "doom-mcp-linux-arm64",
  "darwin-x64": "doom-mcp-darwin-x64",
  "darwin-arm64": "doom-mcp-darwin-arm64",
  "win32-x64": "doom-mcp-win32-x64.exe",
};

const key = `${platform}-${arch}`;
const name = binaries[key];

if (!name) {
  console.warn(`doom-mcp: No pre-built binary for ${key}. You may need to build from source.`);
  process.exit(0);
}

const binaryPath = path.join(__dirname, "..", "engine", name);

if (fs.existsSync(binaryPath)) {
  // Ensure executable permission on unix
  if (platform !== "win32") {
    try {
      fs.chmodSync(binaryPath, 0o755);
    } catch (e) {
      // ignore
    }
  }
  console.log(`doom-mcp: Ready! Binary found for ${key}.`);
} else {
  console.warn(`doom-mcp: Binary not found for ${key} at ${binaryPath}`);
  console.warn(`doom-mcp: Visit https://github.com/gunnargrosch/doom-mcp/releases for manual install.`);
}
