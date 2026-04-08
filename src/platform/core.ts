import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getElectronBridge } from "./electronBridge";

export const invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  const electronInvoke = getElectronBridge()?.invoke;
  if (electronInvoke) {
    return electronInvoke<T>(command, args);
  }

  return tauriInvoke<T>(command, args);
};

