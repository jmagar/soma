#!/usr/bin/env node

const { spawn } = require("node:child_process");

const binary = process.env.RTEMPLATE_BIN || "rtemplate";
const child = spawn(binary, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});

child.on("error", (error) => {
  if (error.code === "ENOENT") {
    console.error(
      `Unable to find ${binary}. Install the rmcp-template binary or set RTEMPLATE_BIN=/path/to/rtemplate.`
    );
    process.exit(127);
  }
  console.error(error.message);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
