import { describe, expect, it, vi } from "vitest";
import { createSettingsQueue, opensExpanded, singleFlight, type SettingsStore } from "./state";
import { DEFAULT_SETTINGS, demoSnapshot } from "./dev/demo";
import type { Settings, Snapshot } from "./types";

describe("Popover opens expanded", () => {
  it("only when the setting is true", () => {
    expect(opensExpanded({ ...DEFAULT_SETTINGS, openExpanded: true })).toBe(true);
  });
  it("not when the setting is false or null", () => {
    expect(opensExpanded({ ...DEFAULT_SETTINGS, openExpanded: false })).toBe(false);
    expect(opensExpanded({ ...DEFAULT_SETTINGS, openExpanded: null })).toBe(false);
  });
});

function deferred<T = void>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function harness(initial: Snapshot, save: (s: Settings) => Promise<void>) {
  const store = {
    current: initial as Snapshot | null,
    writes: [] as Snapshot[],
  };
  const settingsStore: SettingsStore = {
    read: () => store.current,
    write: (next) => { store.current = next; store.writes.push(next); },
    save,
  };
  return { update: createSettingsQueue(settingsStore), store };
}

const initial = demoSnapshot("onpace", DEFAULT_SETTINGS);

describe("settings queue", () => {
  it("applies the patch at once and keeps it when the save succeeds", async () => {
    const save = vi.fn(async () => {});
    const { update, store } = harness(initial, save);
    await update({ theme: "dark" });
    expect(store.writes).toHaveLength(1);
    expect(store.current?.settings.theme).toBe("dark");
    expect(save).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, theme: "dark" });
  });

  it("rolls back to the snapshot from before the patch when the save fails", async () => {
    const { update, store } = harness(initial, () => Promise.reject(new Error("disk full")));
    await update({ notifications: { ...DEFAULT_SETTINGS.notifications, burstAic: 25 } });
    expect(store.writes).toHaveLength(2);
    expect(store.writes[0].settings.notifications.burstAic).toBe(25);
    expect(store.current).toBe(initial);
    expect(store.current?.settings.notifications.burstAic).toBe(10);
  });

  it("bases each patch on the settings already written, so earlier changes stay", async () => {
    const save = vi.fn(async () => {});
    const { update } = harness(initial, save);
    await update({ theme: "dark" });
    await update({ textScale: 1.15 });
    expect(save).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, theme: "dark", textScale: 1.15 });
  });

  it("runs one save at a time, in order", async () => {
    const first = deferred();
    const save = vi.fn(() => first.promise);
    const { update } = harness(initial, save);
    const a = update({ theme: "dark" });
    const b = update({ palette: "colorblind" });
    await Promise.resolve();
    await Promise.resolve();
    expect(save).toHaveBeenCalledTimes(1);
    first.resolve();
    await Promise.all([a, b]);
    expect(save).toHaveBeenCalledTimes(2);
    expect(save).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, theme: "dark", palette: "colorblind" });
  });

  it("a failed save does not stop the next patch from being saved", async () => {
    const save = vi.fn()
      .mockRejectedValueOnce(new Error("busy"))
      .mockResolvedValueOnce(undefined);
    const { update, store } = harness(initial, save);
    await update({ theme: "dark" });
    await update({ weekStartsMonday: false });
    expect(save).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, weekStartsMonday: false });
    expect(store.current?.settings.theme).toBe("system");
    expect(store.current?.settings.weekStartsMonday).toBe(false);
  });

  it("does nothing when there is no snapshot yet", async () => {
    const save = vi.fn(async () => {});
    const { update } = harness(null as unknown as Snapshot, save);
    await update({ theme: "dark" });
    expect(save).not.toHaveBeenCalled();
  });
});

describe("single-flight refresh", () => {
  it("runs the task once while it is in flight and shares its promise", async () => {
    const gate = deferred();
    const task = vi.fn(() => gate.promise);
    const run = singleFlight(task);
    const a = run();
    const b = run();
    expect(task).toHaveBeenCalledTimes(1);
    expect(b).toBe(a);
    gate.resolve();
    await a;
    await run();
    expect(task).toHaveBeenCalledTimes(2);
  });

  it("can run again after the task fails", async () => {
    const task = vi.fn()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce("ok");
    const run = singleFlight(task);
    await expect(run()).rejects.toThrow("offline");
    await expect(run()).resolves.toBe("ok");
    expect(task).toHaveBeenCalledTimes(2);
  });
});
