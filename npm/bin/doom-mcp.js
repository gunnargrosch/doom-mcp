#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

function getBinaryName() {
  const platform = os.platform();
  const arch = os.arch();
  const key = `${platform}-${arch}`;

  const binaries = {
    "linux-x64": "doom-mcp-linux-x64",
    "linux-arm64": "doom-mcp-linux-arm64",
    "darwin-x64": "doom-mcp-darwin-x64",
    "darwin-arm64": "doom-mcp-darwin-arm64",
    "win32-x64": "doom-mcp-win32-x64.exe",
  };

  const name = binaries[key];
  if (!name) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported: ${Object.keys(binaries).join(", ")}`);
    process.exit(1);
  }

  return name;
}

function findWad() {
  const npmWad = path.join(__dirname, "..", "wad", "freedoom1.wad");
  if (fs.existsSync(npmWad)) return npmWad;

  // Check env var
  if (process.env.DOOM_WAD_PATH && fs.existsSync(process.env.DOOM_WAD_PATH)) {
    return process.env.DOOM_WAD_PATH;
  }

  return npmWad; // let the engine handle the error
}

const binaryName = getBinaryName();
const binaryPath = path.join(__dirname, "..", "engine", binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(`Binary not found: ${binaryPath}`);
  console.error(`Run 'npm rebuild doom-mcp' or check your platform is supported.`);
  process.exit(1);
}

// Pass WAD path as env var so the engine can find it
const wadPath = findWad();

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: {
    ...process.env,
    DOOM_WAD_PATH: wadPath,
  },
});

child.on("exit", (code) => {
  process.exit(code || 0);
});

child.on("error", (err) => {
  console.error(`Failed to start doom-mcp: ${err.message}`);
  process.exit(1);
});
