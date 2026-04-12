/**
 * Mock for @tauri-apps/plugin-dialog — browser-mode development.
 */

export async function ask(
  _message: string,
  _options?: unknown,
): Promise<boolean> {
  console.debug("[mock] dialog.ask() → true");
  return true;
}

export async function message(
  _message: string,
  _options?: unknown,
): Promise<void> {
  console.debug("[mock] dialog.message()");
}

export async function confirm(
  _message: string,
  _options?: unknown,
): Promise<boolean> {
  console.debug("[mock] dialog.confirm() → true");
  return true;
}

export async function open(_options?: unknown): Promise<string | string[] | null> {
  console.debug("[mock] dialog.open() → null");
  return null;
}

export async function save(_options?: unknown): Promise<string | null> {
  console.debug("[mock] dialog.save() → null");
  return null;
}
