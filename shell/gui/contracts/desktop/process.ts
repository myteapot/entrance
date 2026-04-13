import { relaunch as tauriRelaunch } from "@tauri-apps/plugin-process";
import { getElectronBridge } from "./electronBridge";

export const relaunch = (): Promise<void> => {
  const electronRelaunch = getElectronBridge()?.process?.relaunch;
  if (electronRelaunch) {
    return electronRelaunch();
  }

  return tauriRelaunch();
};

