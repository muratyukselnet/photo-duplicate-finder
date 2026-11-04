import { beforeEach, describe, expect, it, vi } from "vitest";
import { confirm } from "@tauri-apps/plugin-dialog";
import { withConfirmation } from "./confirmation";
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));

describe("native confirmation safety", () => {
  beforeEach(() => vi.clearAllMocks());
  it("waits for the native answer and never mutates after Cancel", async () => {
    let answer!: (value: boolean) => void;
    vi.mocked(confirm).mockReturnValue(new Promise<boolean>((resolve) => { answer = resolve; }));
    const mutate = vi.fn(async () => {});
    const action = withConfirmation("Trash photos?", { kind: "warning" }, mutate);
    await Promise.resolve();
    expect(mutate).not.toHaveBeenCalled();
    answer(false);
    expect(await action).toBe(false);
    expect(mutate).not.toHaveBeenCalled();
  });
  it("runs the mutation exactly once after confirmation", async () => {
    vi.mocked(confirm).mockResolvedValue(true);
    const mutate = vi.fn(async () => {});
    expect(await withConfirmation("Trash photos?", {}, mutate)).toBe(true);
    expect(mutate).toHaveBeenCalledOnce();
  });
  it("does not mutate when the native dialog fails", async () => {
    vi.mocked(confirm).mockRejectedValue(new Error("Dialog unavailable"));
    const mutate = vi.fn(async () => {});
    await expect(withConfirmation("Trash photos?", {}, mutate)).rejects.toThrow("Dialog unavailable");
    expect(mutate).not.toHaveBeenCalled();
  });
});
