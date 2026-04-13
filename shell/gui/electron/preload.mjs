import { contextBridge, ipcRenderer } from "electron";

const listenOnChannel = (channel, handler) => {
  const listener = (_event, payload) => handler({ payload });
  ipcRenderer.on(channel, listener);
  return Promise.resolve(() => {
    ipcRenderer.removeListener(channel, listener);
  });
};

contextBridge.exposeInMainWorld("__ENTRANCE_ELECTRON__", {
  invoke: (command, args) => ipcRenderer.invoke("entrance:core:invoke", command, args),
  listen: (event, handler) => listenOnChannel(`entrance:event:${event}`, handler),
  dialog: {
    open: (options) => ipcRenderer.invoke("entrance:dialog:open", options),
    ask: (message, options) =>
      ipcRenderer.invoke("entrance:dialog:ask", message, options),
    message: (message, options) =>
      ipcRenderer.invoke("entrance:dialog:message", message, options),
  },
  process: {
    relaunch: () => ipcRenderer.invoke("entrance:process:relaunch"),
  },
  updater: {
    check: (options) => ipcRenderer.invoke("entrance:updater:check", options),
  },
  window: {
    current: () => ({
      show: () => ipcRenderer.invoke("entrance:window:show"),
      hide: () => ipcRenderer.invoke("entrance:window:hide"),
      center: () => ipcRenderer.invoke("entrance:window:center"),
      setFocus: () => ipcRenderer.invoke("entrance:window:set-focus"),
      isVisible: () => ipcRenderer.invoke("entrance:window:is-visible"),
      listen: (event, handler) =>
        listenOnChannel(`entrance:event:${event}`, handler),
      onFocusChanged: (handler) =>
        listenOnChannel("entrance:event:window-focus", handler),
    }),
  },
});
