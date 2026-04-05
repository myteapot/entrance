/**
 * Mock for @tauri-apps/api/window — browser-mode development.
 */

class MockWindow {
  label = "main";

  async listen(_event: string, _handler: unknown): Promise<() => void> {
    return () => {};
  }

  async onFocusChanged(_handler: unknown): Promise<() => void> {
    return () => {};
  }

  async show(): Promise<void> {}
  async hide(): Promise<void> {}
  async setFocus(): Promise<void> {}
  async center(): Promise<void> {}
  async isVisible(): Promise<boolean> { return true; }
  async isFocused(): Promise<boolean> { return true; }
}

const mainWindow = new MockWindow();

export function getCurrentWindow(): MockWindow {
  return mainWindow;
}

export function getAllWindows(): MockWindow[] {
  return [mainWindow];
}

export class Window extends MockWindow {}
