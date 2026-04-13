import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { app, BrowserWindow, dialog, ipcMain } from "electron";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const packagedBridgeName =
  process.platform === "win32" ? "entrance-desktop-bridge.exe" : "entrance-desktop-bridge";

const isDev = Boolean(process.env.ENTRANCE_RENDERER_URL);
const defaultRendererUrl = "http://127.0.0.1:1420";
const bridgeReadyTimeoutMs = 180_000;
const openDevtoolsInDev = process.env.ENTRANCE_ELECTRON_NO_DEVTOOLS !== "1";

let mainWindow;
let bridgeProcess = null;
let bridgeStartPromise = null;
let bridgeStdoutBuffer = "";
let nextBridgeRequestId = 1;
const pendingBridgeInvocations = new Map();

const resolveRendererTarget = () => {
  if (process.env.ENTRANCE_RENDERER_URL) {
    return process.env.ENTRANCE_RENDERER_URL;
  }

  return new URL("../dist/index.html", import.meta.url).toString();
};

const resolveWindow = (webContents) =>
  BrowserWindow.fromWebContents(webContents) ?? mainWindow;

const openDialogResult = (result, options) => {
  if (result.canceled) {
    return null;
  }

  if (options && typeof options === "object" && "multiple" in options && options.multiple) {
    return result.filePaths;
  }

  return result.filePaths[0] ?? null;
};

const toMessageBoxType = (kind) => {
  if (kind === "error") {
    return "error";
  }

  if (kind === "warning") {
    return "warning";
  }

  return "info";
};

const resolveMessageBoxTitle = (options) => {
  if (typeof options === "string") {
    return options;
  }

  return options?.title ?? "Entrance";
};

const resolveMessageBoxKind = (options) => {
  if (typeof options === "string") {
    return "info";
  }

  return options?.kind;
};

const normalizeOpenDialogOptions = (options = {}) => {
  const properties = new Set(Array.isArray(options.properties) ? options.properties : []);

  if (options.directory) {
    properties.add("openDirectory");
  } else {
    properties.add("openFile");
  }

  if (options.multiple) {
    properties.add("multiSelections");
  }

  if (options.showHiddenFiles) {
    properties.add("showHiddenFiles");
  }

  if (options.createDirectory) {
    properties.add("createDirectory");
  }

  if (options.promptToCreate) {
    properties.add("promptToCreate");
  }

  return {
    title: options.title,
    defaultPath: options.defaultPath,
    filters: options.filters,
    buttonLabel: options.buttonLabel,
    message: options.message,
    securityScopedBookmarks: options.securityScopedBookmarks,
    properties: [...properties],
  };
};

const broadcastRendererEvent = (channel, payload) => {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) {
      window.webContents.send(channel, payload);
    }
  }
};

const failPendingInvocations = (error) => {
  for (const pending of pendingBridgeInvocations.values()) {
    pending.reject(error);
  }
  pendingBridgeInvocations.clear();
};

const resolveBridgeSpawn = () => {
  if (app.isPackaged) {
    const packagedBridgePath = path.join(process.resourcesPath, packagedBridgeName);
    if (existsSync(packagedBridgePath)) {
      return {
        command: packagedBridgePath,
        args: ["stdio"],
      };
    }
  }

  const devBridgeCandidates = [
    path.join(repoRoot, "target", "debug", packagedBridgeName),
    path.join(repoRoot, "target", "release", packagedBridgeName),
  ];
  for (const candidate of devBridgeCandidates) {
    if (existsSync(candidate)) {
      return {
        command: candidate,
        args: ["stdio"],
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
      "entrance-gui",
      "--bin",
      "entrance-desktop-bridge",
      "--",
      "stdio",
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

  if (payload.kind === "event" && typeof payload.topic === "string") {
    broadcastRendererEvent(`entrance:event:${payload.topic}`, payload.payload ?? null);
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

  ipcMain.handle("entrance:dialog:open", async (event, options = {}) => {
    const targetWindow = resolveWindow(event.sender);
    const result = await dialog.showOpenDialog(
      targetWindow,
      normalizeOpenDialogOptions(options),
    );
    return openDialogResult(result, options);
  });

  ipcMain.handle("entrance:dialog:ask", async (event, message, options = {}) => {
    const targetWindow = resolveWindow(event.sender);
    const result = await dialog.showMessageBox(targetWindow, {
      type: toMessageBoxType(resolveMessageBoxKind(options)),
      title: resolveMessageBoxTitle(options),
      message,
      buttons: [options.okLabel ?? "Yes", options.cancelLabel ?? "No"],
      defaultId: 0,
      cancelId: 1,
      noLink: true,
    });
    return result.response === 0;
  });

  ipcMain.handle("entrance:dialog:message", async (event, message, options = {}) => {
    const targetWindow = resolveWindow(event.sender);
    await dialog.showMessageBox(targetWindow, {
      type: toMessageBoxType(resolveMessageBoxKind(options)),
      title: resolveMessageBoxTitle(options),
      message,
      buttons: [options.buttons?.ok ?? "OK"],
      defaultId: 0,
      noLink: true,
    });
    return "Ok";
  });

  ipcMain.handle("entrance:process:relaunch", async () => {
    app.relaunch();
    app.exit(0);
  });

  ipcMain.handle("entrance:updater:check", async () => null);

  ipcMain.handle("entrance:window:show", async (event) => {
    resolveWindow(event.sender)?.show();
  });

  ipcMain.handle("entrance:window:hide", async (event) => {
    resolveWindow(event.sender)?.hide();
  });

  ipcMain.handle("entrance:window:center", async (event) => {
    resolveWindow(event.sender)?.center();
  });

  ipcMain.handle("entrance:window:set-focus", async (event) => {
    resolveWindow(event.sender)?.focus();
  });

  ipcMain.handle("entrance:window:is-visible", async (event) =>
    resolveWindow(event.sender)?.isVisible() ?? false,
  );
};

const createMainWindow = async () => {
  mainWindow = new BrowserWindow({
    width: 1120,
    height: 720,
    minWidth: 960,
    minHeight: 640,
    backgroundColor: "#111417",
    title: "Entrance (Electron Adapter)",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      sandbox: false,
      nodeIntegration: false,
    },
  });

  mainWindow.on("focus", () => broadcastRendererEvent("entrance:event:window-focus", true));
  mainWindow.on("blur", () => broadcastRendererEvent("entrance:event:window-focus", false));
  mainWindow.webContents.on("before-input-event", (_event, input) => {
    if (input.type === "keyDown" && input.control && input.key.toLowerCase() === "k") {
      broadcastRendererEvent("entrance:event:launcher:toggle", null);
    }
  });

  const target = resolveRendererTarget();
  if (isDev) {
    await mainWindow.loadURL(target || defaultRendererUrl);
    if (openDevtoolsInDev) {
      mainWindow.webContents.openDevTools({ mode: "detach" });
    }
    return;
  }

  await mainWindow.loadURL(target);
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
