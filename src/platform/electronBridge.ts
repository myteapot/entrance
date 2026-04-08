import type {
  ConfirmDialogOptions,
  MessageDialogOptions,
  MessageDialogResult,
  OpenDialogOptions,
  OpenDialogReturn,
} from "@tauri-apps/plugin-dialog";
import type {
  CheckOptions,
  DownloadEvent,
  DownloadOptions,
} from "@tauri-apps/plugin-updater";

export type DesktopUnlisten = () => void | Promise<void>;

export type DesktopEvent<T = unknown> = {
  payload: T;
};

export type DesktopEventHandler<T = unknown> = (event: DesktopEvent<T>) => void;

export interface DesktopWindowHandle {
  show(): Promise<void>;
  hide(): Promise<void>;
  center(): Promise<void>;
  setFocus(): Promise<void>;
  isVisible(): Promise<boolean>;
  listen<T = unknown>(
    event: string,
    handler: DesktopEventHandler<T>,
  ): Promise<DesktopUnlisten>;
  onFocusChanged(
    handler: DesktopEventHandler<boolean>,
  ): Promise<DesktopUnlisten>;
}

export interface DesktopUpdaterResult {
  version: string;
  body?: string | null;
  downloadAndInstall(
    onEvent?: (progress: DownloadEvent) => void,
    options?: DownloadOptions,
  ): Promise<void>;
}

export interface EntranceElectronBridge {
  invoke?<T = unknown>(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<T>;
  listen?<T = unknown>(
    event: string,
    handler: DesktopEventHandler<T>,
  ): Promise<DesktopUnlisten>;
  dialog?: {
    open?<T extends OpenDialogOptions>(
      options?: T,
    ): Promise<OpenDialogReturn<T>>;
    ask?(message: string, options?: string | ConfirmDialogOptions): Promise<boolean>;
    message?(
      message: string,
      options?: string | MessageDialogOptions,
    ): Promise<MessageDialogResult>;
  };
  process?: {
    relaunch?(): Promise<void>;
  };
  updater?: {
    check?(options?: CheckOptions): Promise<DesktopUpdaterResult | null>;
  };
  window?: {
    current?(): DesktopWindowHandle;
  };
}

declare global {
  interface Window {
    __ENTRANCE_ELECTRON__?: EntranceElectronBridge;
  }
}

export const getElectronBridge = (): EntranceElectronBridge | undefined => {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__ENTRANCE_ELECTRON__;
};
