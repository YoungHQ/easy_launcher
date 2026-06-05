import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const projectRoot = process.cwd();
const localEnv = readLocalEnv(path.join(projectRoot, ".env.local"));
const cargoHome = process.env.CARGO_HOME || localEnv.CARGO_HOME || "";
const rustupHome = process.env.RUSTUP_HOME || localEnv.RUSTUP_HOME || "";
const cargoBin = cargoHome ? path.join(cargoHome, "bin") : "";
const localTauriEntry = path.join(
  projectRoot,
  "node_modules",
  "@tauri-apps",
  "cli",
  "tauri.js",
);

const env = {
  ...localEnv,
  ...process.env,
  PATH: cargoBin
    ? `${cargoBin}${path.delimiter}${process.env.PATH ?? localEnv.PATH ?? ""}`
    : process.env.PATH,
};

if (cargoHome) {
  env.CARGO_HOME = cargoHome;
}

if (rustupHome) {
  env.RUSTUP_HOME = rustupHome;
}

if (!existsSync(localTauriEntry)) {
  console.error(`Tauri CLI not found: ${localTauriEntry}`);
  process.exit(1);
}

const cargoCommand = process.platform === "win32" ? "cargo.exe" : "cargo";
const configuredCargo = cargoBin ? path.join(cargoBin, cargoCommand) : "";

if (configuredCargo && !existsSync(configuredCargo)) {
  console.error(`Configured Cargo not found: ${configuredCargo}`);
  console.error("Install Rust, set CARGO_HOME, or make cargo available on PATH.");
  process.exit(1);
}

const child = spawn(process.execPath, [localTauriEntry, ...process.argv.slice(2)], {
  cwd: projectRoot,
  env,
  stdio: "inherit",
  shell: false,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 0);
});

function readLocalEnv(filePath) {
  if (!existsSync(filePath)) {
    return {};
  }

  const values = {};
  const content = readFileSync(filePath, "utf8");
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const separator = trimmed.indexOf("=");
    if (separator <= 0) {
      continue;
    }

    const key = trimmed.slice(0, separator).trim();
    const rawValue = trimmed.slice(separator + 1).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
      continue;
    }

    values[key] = stripEnvQuotes(rawValue);
  }

  return values;
}

function stripEnvQuotes(value) {
  if (value.length < 2) {
    return value;
  }

  const first = value[0];
  const last = value[value.length - 1];
  if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
    return value.slice(1, -1);
  }

  return value;
}
