import { confirm, type ConfirmDialogOptions } from "@tauri-apps/plugin-dialog";

/** Tauri replaces window.confirm with an async function; await the native decision. */
export async function withConfirmation(
  message: string,
  options: ConfirmDialogOptions,
  action: () => Promise<void>,
): Promise<boolean> {
  if (!(await confirm(message, options))) return false;
  await action();
  return true;
}
