/**
 * Mock for @tauri-apps/plugin-process — browser-mode development.
 */

export async function relaunch(): Promise<void> {
  console.debug("[mock] process.relaunch() — reloading page instead");
  window.location.reload();
}

export async function exit(_code?: number): Promise<void> {
  console.debug("[mock] process.exit()");
}
