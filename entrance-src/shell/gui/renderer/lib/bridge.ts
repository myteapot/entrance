export type InvokeArgs = Record<string, unknown>;

export type ElectronBridge = {
  invoke<T = unknown>(command: string, args?: InvokeArgs): Promise<T>;
};

declare global {
  interface Window {
    __ENTRANCE_ELECTRON__?: ElectronBridge;
  }
}

export const bridge = {
  async invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
    const electron = window.__ENTRANCE_ELECTRON__;
    if (!electron?.invoke) {
      throw new Error("Electron bridge is not available");
    }
    return electron.invoke<T>(command, args);
  },
};
