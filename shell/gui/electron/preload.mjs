import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("__ENTRANCE_ELECTRON__", {
  invoke: (command, args) => ipcRenderer.invoke("entrance:core:invoke", command, args),
});
