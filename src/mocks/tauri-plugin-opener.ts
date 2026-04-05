/**
 * Mock for @tauri-apps/plugin-opener — browser-mode development.
 */

export async function open(url: string): Promise<void> {
  console.debug(`[mock] opener.open("${url}")`);
  window.open(url, "_blank", "noreferrer");
}

export async function reveal(path: string): Promise<void> {
  console.debug(`[mock] opener.reveal("${path}") — not supported in browser`);
}
