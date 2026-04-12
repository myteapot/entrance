import {
  check as tauriCheck,
  type CheckOptions,
} from "@tauri-apps/plugin-updater";
import {
  getElectronBridge,
  type DesktopUpdaterResult,
} from "./electronBridge";

export const check = (
  options?: CheckOptions,
): Promise<DesktopUpdaterResult | null> => {
  const electronCheck = getElectronBridge()?.updater?.check;
  if (electronCheck) {
    return electronCheck(options);
  }

  return tauriCheck(options);
};
