import { invoke } from "../platform/core";
import { For, Show } from "solid-js";

import type { NotaDialogStore } from "../features/dashboard/notaDialogStore";
import "./NotaDialog.css";

interface NotaDialogProps {
  store: NotaDialogStore;
}

const NotaDialog = (props: NotaDialogProps) => {
  const handleAction = async (
    dialogId: string,
    actionKey: string,
    allocationId: number | null,
  ) => {
    if (allocationId != null) {
      if (actionKey === "approve") {
        await invoke("nota_approve_prayer", { allocationId });
      } else if (actionKey === "reject") {
        await invoke("nota_reject_prayer", { allocationId, reason: actionKey });
      }
    }
    props.store.dismiss(dialogId);
  };

  return (
    <Show when={props.store.current()}>
      {(dialog) => (
        <div class="nota-dialog-overlay">
          <div class="nota-dialog-header">
            <span class="nota-dialog-icon" />
            <span class="nota-dialog-kind">{dialog().kind}</span>
            <button
              type="button"
              class="nota-dialog-close"
              onClick={() => props.store.dismiss(dialog().dialog_id)}
              title="Defer"
            >
              ×
            </button>
          </div>
          <h4 class="nota-dialog-title">{dialog().title}</h4>
          <p class="nota-dialog-body">{dialog().body}</p>
          <Show when={props.store.pendingCount() > 1}>
            <span class="nota-dialog-badge">{props.store.pendingCount()} pending</span>
          </Show>
          <div class="nota-dialog-actions">
            <For each={dialog().actions}>
              {(action) => (
                <button
                  type="button"
                  class={`nota-dialog-action nota-dialog-action--${action.tone}`}
                  onClick={() =>
                    void handleAction(dialog().dialog_id, action.action_key, dialog().allocation_id)
                  }
                >
                  {action.label}
                </button>
              )}
            </For>
          </div>
        </div>
      )}
    </Show>
  );
};

export default NotaDialog;
