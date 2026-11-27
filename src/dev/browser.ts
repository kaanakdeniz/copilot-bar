import { installBrowserBackend } from "../backend";
import type { Settings, Snapshot, UpdateState } from "../types";
import { allowanceOf, buildDemo, computePace, DEFAULT_SETTINGS, type DemoOpts, demoSnapshot, overageOf, type Scenario, SCENARIOS } from "./demo";

export interface StudioConfig { data: DemoOpts; settings: Partial<Settings> }

const params = new URLSearchParams(location.search);
const STUDIO_LAST = "copilotbar.studio.last";
const CHECK_DELAY_MS = 600;

let mock: Snapshot | null = null;
let mockUpdate: UpdateState = { state: "idle" };
const mockUpdateListeners = new Set<(u: UpdateState) => void>();
const mockListeners = new Set<(s: Snapshot) => void>();

function setMockUpdate(u: UpdateState) {
  mockUpdate = u;
  for (const l of mockUpdateListeners) l(u);
}
function emitMock() {
  for (const l of mockListeners) l(mock!);
}

function fromStudio(c: StudioConfig): Snapshot {
  return buildDemo(c.data, { ...DEFAULT_SETTINGS, ...c.settings });
}

if (params.has("studio")) {
  window.addEventListener("message", (e) => {
    if (e.origin !== location.origin || e.data?.type !== "studio-config") return;
    mock = fromStudio(e.data.config as StudioConfig);
    try { localStorage.setItem(STUDIO_LAST, JSON.stringify(e.data.config)); } catch {}
    emitMock();
  });
  window.parent?.postMessage({ type: "studio-ready" }, location.origin);
}

function overrides(): Partial<Settings> {
  const o: Partial<Settings> = {};
  const pal = params.get("palette");
  if (pal === "colorblind") o.palette = pal;
  const zoom = params.get("zoom");
  if (zoom) o.textScale = Number(zoom);
  const theme = params.get("theme");
  if (theme === "light" || theme === "dark") o.theme = theme;
  return o;
}

async function mockSnapshot(): Promise<Snapshot> {
  if (mock) return mock;
  const cfg = params.get("cfg");
  if (cfg) { try { return (mock = fromStudio(JSON.parse(atob(cfg)))); } catch {} }
  if (params.has("studio") && params.get("view") === "dashboard") {
    try { const c = localStorage.getItem(STUDIO_LAST); if (c) return (mock = fromStudio(JSON.parse(c))); } catch {}
  }
  if (params.has("studio") && params.get("view") !== "dashboard") {
    return new Promise((res) => { const once = (s: Snapshot) => { mockListeners.delete(once); res(s); }; mockListeners.add(once); });
  }
  const file = params.get("snapshot");
  if (file) {
    const s: Snapshot = await (await fetch(file)).json();
    s.settings = { ...DEFAULT_SETTINGS, ...s.settings, ...overrides() };
    mock = s;
  } else {
    const scn = (params.get("demo") ?? "onpace") as Scenario;
    mock = demoSnapshot(scn in SCENARIOS ? scn : "onpace", { ...DEFAULT_SETTINGS, ...overrides() });
  }
  return mock;
}

installBrowserBackend({
  async defaultSettings() {
    return DEFAULT_SETTINGS;
  },
  snapshot: mockSnapshot,
  async refresh() {
    const s = await mockSnapshot();
    mock = { ...s, status: { ...s.status, lastRefresh: s.now } };
    emitMock();
    return mock;
  },
  async saveSettings(settings) {
    const s = await mockSnapshot();
    if (params.has("studio") && window.parent !== window) {
      const patch = Object.fromEntries(Object.entries(settings).filter(([k, v]) =>
        k !== "theme" && k !== "allowanceOverride" && JSON.stringify(v) !== JSON.stringify((s.settings as unknown as Record<string, unknown>)[k])));
      if (Object.keys(patch).length) window.parent.postMessage({ type: "studio-settings", patch }, location.origin);
    }
    const { allowance, ...source } = allowanceOf(settings.allowanceOverride, s.githubPlan);
    const pace = computePace(s.records, s.pace.cycle, allowance, s.githubPlan?.creditsUsed ?? null, s.now, settings.paceTolerancePct / 100);
    mock = { ...s, settings, ...source, pace, ...overageOf(s.githubPlan, pace.used) };
    emitMock();
  },
  async updateState() {
    return mockUpdate;
  },
  async checkForUpdates() {
    setMockUpdate({ state: "checking" });
    await new Promise((r) => setTimeout(r, CHECK_DELAY_MS));
    setMockUpdate({ state: "upToDate" });
    return mockUpdate;
  },
  onSnapshot(cb) {
    mockListeners.add(cb);
    return () => void mockListeners.delete(cb);
  },
  onUpdate(cb) {
    mockUpdateListeners.add(cb);
    return () => void mockUpdateListeners.delete(cb);
  },
});
