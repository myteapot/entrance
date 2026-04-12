import { listen as tauriListen } from "@tauri-apps/api/event";
import {
  getElectronBridge,
  type DesktopEventHandler,
  type DesktopUnlisten,
} from "./electronBridge";

export const listen = <T>(
  event: string,
  handler: DesktopEventHandler<T>,
): Promise<DesktopUnlisten> => {
  const electronListen = getElectronBridge()?.listen;
  if (electronListen) {
    return electronListen<T>(event, handler);
  }

  return tauriListen<T>(event, handler);
};

