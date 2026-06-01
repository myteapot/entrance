export type InvokeArgs = Record<string, unknown>;

export type ElectronBridge = {
  invoke<T = unknown>(command: string, args?: InvokeArgs): Promise<T>;
};

type HttpInvokeResponse<T> = {
  ok: boolean;
  id: string;
  result?: T;
  error?: string;
};

declare global {
  interface Window {
    __ENTRANCE_ELECTRON__?: ElectronBridge;
  }
}

const httpBaseUrl = import.meta.env.VITE_ENTRANCE_HTTP_URL ?? "http://127.0.0.1:9720";
let nextHttpRequestId = 1;

export const bridge = {
  async invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
    const electron = window.__ENTRANCE_ELECTRON__;
    if (!electron?.invoke) {
      const response = await fetch(`${httpBaseUrl}/invoke`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          id: String(nextHttpRequestId++),
          command,
          args: args ?? {},
        }),
      });
      const payload = (await response.json()) as HttpInvokeResponse<T>;
      if (!payload.ok) {
        throw new Error(payload.error ?? "Entrance HTTP bridge request failed");
      }
      return payload.result as T;
    }
    return electron.invoke<T>(command, args);
  },
};
