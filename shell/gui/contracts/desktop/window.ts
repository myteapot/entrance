import { getCurrentWindow as tauriGetCurrentWindow } from "@tauri-apps/api/window";
import {
  getElectronBridge,
  type DesktopWindowHandle,
} from "./electronBridge";

export const getCurrentWindow = (): DesktopWindowHandle => {
  const electronWindow = getElectronBridge()?.window?.current;
  if (electronWindow) {
    return electronWindow();
  }

  return tauriGetCurrentWindow();
};

