import {
  ask as tauriAsk,
  message as tauriMessage,
  open as tauriOpen,
  type ConfirmDialogOptions,
  type MessageDialogOptions,
  type MessageDialogResult,
  type OpenDialogOptions,
  type OpenDialogReturn,
} from "@tauri-apps/plugin-dialog";
import { getElectronBridge } from "./electronBridge";

export const open = <T extends OpenDialogOptions>(
  options?: T,
): Promise<OpenDialogReturn<T>> => {
  const electronOpen = getElectronBridge()?.dialog?.open;
  if (electronOpen) {
    return electronOpen(options);
  }

  return tauriOpen(options);
};

export const ask = (
  message: string,
  options?: string | ConfirmDialogOptions,
): Promise<boolean> => {
  const electronAsk = getElectronBridge()?.dialog?.ask;
  if (electronAsk) {
    return electronAsk(message, options);
  }

  return tauriAsk(message, options);
};

export const message = (
  messageText: string,
  options?: string | MessageDialogOptions,
): Promise<MessageDialogResult> => {
  const electronMessage = getElectronBridge()?.dialog?.message;
  if (electronMessage) {
    return electronMessage(messageText, options);
  }

  return tauriMessage(messageText, options);
};
