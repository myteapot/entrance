/**
 * Mock for @tauri-apps/plugin-updater — browser-mode development.
 */

export async function check(_options?: unknown): Promise<null> {
  console.debug("[mock] updater.check() → null (no update)");
  return null;
}
