import { createSignal } from "solid-js";

import type { NotaDialogEvent } from "./graphEvents";

const readEnabledPreference = () =>
  typeof window === "undefined"
    ? true
    : window.localStorage.getItem("nota_dialog_enabled") !== "false";

export function createNotaDialogStore() {
  const [queue, setQueue] = createSignal<NotaDialogEvent[]>([]);
  const [enabled, setEnabled] = createSignal(readEnabledPreference());

  const toggleEnabled = (value: boolean) => {
    setEnabled(value);
    if (typeof window !== "undefined") {
      window.localStorage.setItem("nota_dialog_enabled", String(value));
    }
  };

  const push = (event: NotaDialogEvent) => {
    if (!enabled()) {
      return;
    }

    setQueue((current) => [...current, event]);
  };

  const dismiss = (dialogId: string) => {
    setQueue((current) => current.filter((dialog) => dialog.dialog_id !== dialogId));
  };

  const current = () => queue()[0] ?? null;
  const pendingCount = () => queue().length;

  return { current, pendingCount, push, dismiss, enabled, toggleEnabled };
}

export type NotaDialogStore = ReturnType<typeof createNotaDialogStore>;
