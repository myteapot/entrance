import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { app, BrowserWindow, dialog, ipcMain } from "electron";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const packagedBinaryName =
  process.platform === "win32" ? "entrance.exe" : "entrance";

const isDev = Boolean(process.env.ENTRANCE_RENDERER_URL);
const bridgeReadyTimeoutMs = 180_000;
const openDevtoolsInDev = process.env.ENTRANCE_ELECTRON_NO_DEVTOOLS !== "1";

let mainWindow;
let bridgeProcess = null;
let bridgeStartPromise = null;
let bridgeStdoutBuffer = "";
let nextBridgeRequestId = 1;
const pendingBridgeInvocations = new Map();

const failPendingInvocations = (error) => {
  for (const pending of pendingBridgeInvocations.values()) {
    pending.reject(error);
  }
  pendingBridgeInvocations.clear();
};

const resolveBridgeSpawn = () => {
  if (app.isPackaged) {
    const packagedBinaryPath = path.join(process.resourcesPath, packagedBinaryName);
    if (existsSync(packagedBinaryPath)) {
      return {
        command: packagedBinaryPath,
        args: ["daemon"],
      };
    }
  }

  const devBinaryCandidates = [
    path.join(repoRoot, "target", "debug", packagedBinaryName),
    path.join(repoRoot, "target", "release", packagedBinaryName),
  ];
  for (const candidate of devBinaryCandidates) {
    if (existsSync(candidate)) {
      return {
        command: candidate,
        args: ["daemon"],
      };
    }
  }

  return {
    command: "cargo",
    args: [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "entrance-app",
      "--bin",
      "entrance",
      "--",
      "daemon",
    ],
  };
};

const resolveBridgeCwd = () => {
  if (app.isPackaged) {
    // In packaged builds __dirname resolves inside app.asar, which is a file.
    // Use the executable directory as a stable real directory for child process cwd.
    return path.dirname(process.execPath);
  }

  return repoRoot;
};

const handleBridgeMessage = (line) => {
  let payload;
  try {
    payload = JSON.parse(line);
  } catch (error) {
    console.error("[entrance-bridge] Failed to parse stdout payload:", line, error);
    return;
  }

  if (payload.kind === "ready") {
    return;
  }

  if (payload.kind === "response") {
    const pending = pendingBridgeInvocations.get(payload.id);
    if (!pending) {
      return;
    }

    pendingBridgeInvocations.delete(payload.id);
    if (payload.ok) {
      pending.resolve(payload.result ?? null);
      return;
    }

    pending.reject(new Error(payload.error ?? "Electron bridge request failed"));
    return;
  }

  console.warn("[entrance-bridge] Ignoring unknown message:", payload);
};

const stopRustBridge = () => {
  if (!bridgeProcess) {
    return;
  }

  const processToStop = bridgeProcess;
  bridgeProcess = null;
  bridgeStartPromise = null;
  processToStop.kill();
};

const startRustBridge = () => {
  if (bridgeStartPromise) {
    return bridgeStartPromise;
  }

  bridgeStartPromise = new Promise((resolve, reject) => {
    const { command, args } = resolveBridgeSpawn();
    const child = spawn(command, args, {
      cwd: resolveBridgeCwd(),
      env: { ...process.env },
      stdio: ["pipe", "pipe", "pipe"],
    });

    bridgeProcess = child;
    bridgeStdoutBuffer = "";

    let ready = false;
    const readyTimer = setTimeout(() => {
      const error = new Error("Timed out while waiting for the Rust Electron bridge to start");
      reject(error);
      failPendingInvocations(error);
      stopRustBridge();
    }, bridgeReadyTimeoutMs);

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      bridgeStdoutBuffer += chunk;

      while (bridgeStdoutBuffer.includes("\n")) {
        const newlineIndex = bridgeStdoutBuffer.indexOf("\n");
        const line = bridgeStdoutBuffer.slice(0, newlineIndex).trim();
        bridgeStdoutBuffer = bridgeStdoutBuffer.slice(newlineIndex + 1);
        if (!line) {
          continue;
        }

        let message;
        try {
          message = JSON.parse(line);
        } catch {
          handleBridgeMessage(line);
          continue;
        }

        if (message.kind === "ready" && !ready) {
          ready = true;
          clearTimeout(readyTimer);
          resolve();
        }

        handleBridgeMessage(line);
      }
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      process.stderr.write(`[entrance-bridge] ${chunk}`);
    });

    child.on("error", (error) => {
      clearTimeout(readyTimer);
      if (!ready) {
        reject(error);
      }
      failPendingInvocations(error);
      bridgeProcess = null;
      bridgeStartPromise = null;
    });

    child.on("exit", (code, signal) => {
      clearTimeout(readyTimer);
      const error = new Error(
        `Rust Electron bridge exited before completing request processing (code=${code ?? "null"}, signal=${signal ?? "null"})`,
      );
      if (!ready) {
        reject(error);
      }
      failPendingInvocations(error);
      bridgeProcess = null;
      bridgeStartPromise = null;
    });
  }).catch((error) => {
    bridgeProcess = null;
    bridgeStartPromise = null;
    throw error;
  });

  return bridgeStartPromise;
};

const invokeRustBridge = async (command, args = {}) => {
  await startRustBridge();
  if (!bridgeProcess || bridgeProcess.killed || !bridgeProcess.stdin) {
    throw new Error("Rust Electron bridge is not available");
  }

  const id = String(nextBridgeRequestId++);
  const request = {
    kind: "invoke",
    id,
    command,
    args: args ?? {},
  };

  const response = new Promise((resolve, reject) => {
    pendingBridgeInvocations.set(id, { resolve, reject });
  });

  bridgeProcess.stdin.write(`${JSON.stringify(request)}\n`);
  return response;
};

const registerIpc = () => {
  ipcMain.handle("entrance:core:invoke", async (_event, command, args = {}) =>
    invokeRustBridge(command, args),
  );
};

const createMainWindow = async () => {
  mainWindow = new BrowserWindow({
    width: 1120,
    height: 760,
    minWidth: 980,
    minHeight: 680,
    backgroundColor: "#101216",
    title: "Entrance",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      sandbox: false,
      nodeIntegration: false,
    },
  });

  if (isDev) {
    await mainWindow.loadURL(process.env.ENTRANCE_RENDERER_URL ?? "http://127.0.0.1:1420");
    if (openDevtoolsInDev) {
      mainWindow.webContents.openDevTools({ mode: "detach" });
    }
    return;
  }

  await mainWindow.loadURL(new URL("../dist/index.html", import.meta.url).toString());
};

app.whenReady().then(async () => {
  await startRustBridge();
  registerIpc();
  await createMainWindow();

  app.on("activate", async () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      await createMainWindow();
    }
  });
}).catch((error) => {
  dialog.showErrorBox("Entrance Electron bridge failed", String(error));
  app.quit();
});

app.on("before-quit", () => {
  stopRustBridge();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
