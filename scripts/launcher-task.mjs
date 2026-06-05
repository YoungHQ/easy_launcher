import { spawn } from "node:child_process";
import { createInterface } from "node:readline/promises";
import process from "node:process";

const commands = {
  start: {
    label: "启动桌面开发项目",
    args: ["run", "tauri", "--", "dev"],
  },
  msi: {
    label: "构建 Windows MSI",
    args: ["run", "tauri", "--", "build"],
  },
};

const aliases = {
  dev: "start",
  run: "start",
  launch: "start",
  build: "msi",
  package: "msi",
};

const requested = normalizeCommand(process.argv[2]);

if (requested) {
  runTask(requested);
} else {
  const selected = await promptForTask();
  runTask(selected);
}

function normalizeCommand(value) {
  if (!value) {
    return "";
  }

  const command = value.trim().toLowerCase();
  const normalized = aliases[command] ?? command;

  if (commands[normalized]) {
    return normalized;
  }

  console.error(`Unknown task: ${value}`);
  printUsage();
  process.exit(1);
}

async function promptForTask() {
  const rl = createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  try {
    console.log("Choose a task:");
    console.log("  1) Start desktop dev app");
    console.log("  2) Build Windows MSI");
    const answer = await rl.question("Enter 1 or 2: ");

    if (answer.trim() === "1") {
      return "start";
    }
    if (answer.trim() === "2") {
      return "msi";
    }

    console.error(`Invalid choice: ${answer}`);
    process.exit(1);
  } finally {
    rl.close();
  }
}

function runTask(command) {
  const task = commands[command];
  console.log(`> ${task.label}`);
  console.log(`> npm ${task.args.join(" ")}`);

  const child = spawn("npm", task.args, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }

    process.exit(code ?? 0);
  });
}

function printUsage() {
  console.log("Usage:");
  console.log("  npm run task");
  console.log("  npm run task -- start");
  console.log("  npm run task -- msi");
}
