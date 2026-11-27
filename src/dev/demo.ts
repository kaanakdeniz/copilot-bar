
import { DAY, recordsFor, startOfDay } from "../analytics";
import type { Client, Pace, PaceStatus, GitHubPlan, SessionRef, Settings, Snapshot, UsageRecord } from "../types";

const MODELS = [
  { id: "claude-sonnet-4.6", w: 0.38, rate: 9 },
  { id: "claude-opus-4.6", w: 0.27, rate: 30 },
  { id: "gpt-5.6-luna", w: 0.16, rate: 2.5 },
  { id: "gpt-5.4", w: 0.1, rate: 6 },
  { id: "claude-haiku-4.5", w: 0.06, rate: 2 },
  { id: "gemini-3.1-pro", w: 0.03, rate: 7 },
];
export const SCENARIOS = { under: 58, onpace: 137, ahead: 176, exhausted: 206 } as const;
export type Scenario = keyof typeof SCENARIOS;
const PROJECTS = ["copilot-bar", "api-gateway", "web-app", "infra"];
const CLIENT_OF: Record<string, Client> = { "copilot-bar": "cli", "api-gateway": "cli", "web-app": "vscode", infra: "app" };
const TASKS = ["Fix flaky tests", "Add retries", "Refactor settings", "Review PR feedback", "Update dependencies", "Write docs"];
function demoSession(project: string, ts: number): SessionRef {
  const d = new Date(ts);
  const n = d.getDate() + PROJECTS.indexOf(project) * 7;
  return { id: `${project}-${d.getMonth() + 1}-${d.getDate()}`, label: TASKS[n % TASKS.length] };
}

function rng(seed: number) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function computePace(records: UsageRecord[], cycle: { start: number; end: number }, allowance: number | null, reported: number | null, now: number, tolerance: number): Pace {
  const credits = recordsFor(records, "aic");
  const sum = (a: number, b: number) => credits.filter((r) => r.ts >= a && r.ts < b).reduce((s, r) => s + (r.aic ?? 0), 0);
  const loggedAic = sum(cycle.start, Math.min(now, cycle.end));
  const used = Math.max(loggedAic, reported ?? 0);
  const totalDays = (cycle.end - cycle.start) / DAY;
  const elapsed = Math.min(totalDays, Math.max(0, (now - cycle.start) / DAY));
  const remaining = Math.max(0, totalDays - elapsed);
  const avg = used / Math.max(1, elapsed);
  const projected = used + avg * remaining;
  const recentStart = Math.max(cycle.start, now - 7 * DAY);
  const avgRecentRaw = sum(recentStart, now) / Math.max(1, (now - recentStart) / DAY);
  const active = new Set(credits.filter((r) => r.ts >= cycle.start && r.ts < now && (r.aic ?? 0) > 0).map((r) => startOfDay(r.ts))).size;
  const todayStart = Math.max(startOfDay(now), cycle.start);
  const usedToday = sum(todayStart, now);
  const lastHour = credits.filter((r) => r.ts > now - 3_600_000 && r.ts <= now).reduce((s, r) => s + (r.aic ?? 0), 0);
  let status: PaceStatus = "noAllowance", evenPace = null, delta = null, dailyBudget = null, runOutAt = null;
  if (allowance && allowance > 0) {
    evenPace = (allowance * elapsed) / totalDays;
    delta = used - evenPace;
    dailyBudget = Math.max(0, allowance - (used - usedToday)) / Math.max(1, Math.ceil((cycle.end - todayStart) / DAY));
    status = used >= allowance ? "exhausted" : Math.abs(delta) <= allowance * tolerance ? "onPace" : delta > 0 ? "ahead" : "under";
    if (avg > 0 && projected > allowance && used < allowance) runOutAt = cycle.start + (allowance / avg) * DAY;
  }
  return {
    cycle, used, loggedAic, unlogged: Math.max(0, used - loggedAic), allowance: allowance && allowance > 0 ? allowance : null, totalDays, elapsedDays: elapsed, remainingDays: remaining,
    avgDaily: avg, avgRecent: avgRecentRaw * (loggedAic > 0 && used > loggedAic ? used / loggedAic : 1), avgPerActiveDay: active ? used / active : 0, activeDays: active, projected, evenPace, delta,
    usedToday, lastHour, dailyBudget, runOutAt, status, fracUsed: allowance ? used / allowance : null, fracElapsed: totalDays ? elapsed / totalDays : 0,
  };
}

export const DEFAULT_SETTINGS: Settings = {
  openExpanded: null, cycleSummary: "gauge", cycleHeadline: "used", menuBarDisplay: "icon-number", menuBarIcon: "disc", menuBarNumber: "pct", alertColors: true, theme: "system", allowanceOverride: 0,
  paceTolerancePct: 5, weekStartsMonday: true, refreshMinutes: 5, sources: { cli: true },
  cliLogDir: null,
  notifications: { enabled: true, thresholds: [75, 90, 100], aheadOfPace: true, burstAic: 10 }, onboarded: true,
  textScale: 1, palette: "default", hotkey: "Alt+Cmd+C", autoUpdateCheck: true,
};

export function allowanceOf(allowanceOverride: number, githubPlan: GitHubPlan | null): { allowance: number | null } & Pick<Snapshot, "allowanceSource" | "allowanceCapped"> {
  const e = githubPlan && !githubPlan.unlimited && (githubPlan.entitlement ?? 0) > 0 ? githubPlan.entitlement! : null;
  if (allowanceOverride > 0) {
    const capped = e !== null && !githubPlan!.overagePermitted && allowanceOverride > e;
    return { allowance: capped ? e : allowanceOverride, allowanceSource: "custom", allowanceCapped: capped };
  }
  return { allowance: e, allowanceSource: e !== null ? "github" : "none", allowanceCapped: false };
}

export function overageOf(githubPlan: GitHubPlan | null, used: number): Pick<Snapshot, "entitlementUsedUp" | "copilotStopped" | "overage"> {
  const e = githubPlan && !githubPlan.unlimited && (githubPlan.entitlement ?? 0) > 0 ? githubPlan.entitlement! : null;
  const permitted = githubPlan?.overagePermitted ?? false;
  const entitlementUsedUp = e !== null && used >= e;
  return { entitlementUsedUp, copilotStopped: entitlementUsedUp && !permitted, overage: e !== null && permitted ? Math.max(0, used - e) : 0 };
}

export function demoSnapshot(scenario: Scenario, settings: Settings = DEFAULT_SETTINGS): Snapshot {
  const now = new Date(2026, 10, 21, 15, 0).getTime();
  const cycle = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
  const r = rng(42);
  const raw: { ts: number; model: (typeof MODELS)[number]; w: number; project: string; hour: number }[] = [];
  for (let i = 0; i < 30; i++) {
    const day = new Date(2026, 10, 1 + i);
    if (day.getTime() > now) break;
    const wd = day.getDay();
    let f = wd === 0 || wd === 6 ? 0.22 : 0.75 + r() * 0.6;
    if (i === 8 || i === 9) f *= 0.15;
    const isToday = i === 20;
    for (const m of MODELS) {
      const calls = 3 + Math.floor(r() * 4);
      for (let c = 0; c < calls; c++) {
        const hour = isToday ? 9 + Math.floor(r() * 6) : 9 + Math.floor(r() * 10);
        raw.push({ ts: new Date(2026, 10, 1 + i, hour, Math.floor(r() * 60)).getTime(), model: m, w: (f * m.w * (0.55 + r() * 0.9)) / calls * (isToday ? 0.62 : 1), project: PROJECTS[Math.floor(r() * PROJECTS.length)], hour });
      }
    }
  }
  const rawTotal = raw.reduce((s, x) => s + x.w, 0);
  const k = SCENARIOS[scenario] / rawTotal;
  const records: UsageRecord[] = raw.map((x, n) => {
    const aic = x.w * k;
    const tok = (aic / x.model.rate) * 1e6;
    const input = Math.round(tok * 0.97);
    return {
      id: `demo:${n}`, ts: x.ts, client: CLIENT_OF[x.project], model: x.model.id,
      tokens: { input, output: Math.round(tok * 0.03), cacheRead: Math.round(input * 0.8), cacheWrite: Math.round(input * 0.08), reasoning: Math.round(tok * 0.004) },
      aic, requests: Math.max(1, Math.round(tok / 42000)), project: x.project, session: demoSession(x.project, x.ts),
      initiator: r() < 0.7 ? "agent" : "user", reasoningEffort: ["low", "medium", "high"][Math.floor(r() * 3)], isAuto: r() < 0.4,
    };
  });
  const used = SCENARIOS[scenario];
  const githubPlan: GitHubPlan = { login: "octocat", plan: "individual", sku: "copilot_pro", creditsUsed: used, entitlement: 200, overagePermitted: true, unlimited: false, resetAt: cycle.end, fetchedAt: now };
  const { allowance, ...source } = allowanceOf(settings.allowanceOverride, githubPlan);
  const pace = computePace(records, cycle, allowance, used, now, settings.paceTolerancePct / 100);
  return {
    records, now, pace, settings, ...source, githubPlan, ...overageOf(githubPlan, pace.used),
    status: { lastRefresh: now - 40_000, planError: null, signIn: { state: "signedIn", login: "octocat" }, cliFiles: 64, connectingSince: null },
  };
}

export interface DemoOpts {
  used: number;
  allowance: number;
  day: number;
  today: number;
  localShare: number;
  overagePermitted: boolean;
  connection: "ok" | "connecting" | "stale" | "offline";
  history?: "steady" | "peak" | "recent";
  activeWeekends?: boolean;
}

export function buildDemo(o: DemoOpts, settings: Settings): Snapshot {
  const day = Math.min(30, Math.max(1, Math.round(o.day)));
  const now = new Date(2026, 10, day, 15, 0).getTime();
  const cycle = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
  const r = rng(42);
  const loggedAic = Math.max(0, o.used * o.localShare);
  const todayAmt = Math.min(Math.max(0, o.today), loggedAic);
  const pastAmt = loggedAic - todayAmt;
  type Raw = { ts: number; model: (typeof MODELS)[number]; w: number; project: string; today: boolean };
  const raw: Raw[] = [];
  for (let i = 0; i < day; i++) {
    const isToday = i === day - 1;
    const wd = new Date(2026, 10, 1 + i).getDay();
    let f = (wd === 0 || wd === 6) && !o.activeWeekends ? 0.22 : 0.75 + r() * 0.6;
    if (i === 8 || i === 9) f *= 0.15;
    if (o.history === "peak") f *= i === Math.floor((day - 1) * 0.45) ? 14 : 0.5;
    if (o.history === "recent") f *= i >= day - 8 ? 4 : 0.25;
    for (const m of MODELS) {
      const calls = 3 + Math.floor(r() * 4);
      for (let c = 0; c < calls; c++) {
        const hour = isToday ? 9 + Math.floor(r() * 6) : 9 + Math.floor(r() * 10);
        raw.push({ ts: new Date(2026, 10, 1 + i, hour, Math.floor(r() * 60)).getTime(), model: m, w: (f * m.w * (0.55 + r() * 0.9)) / calls, project: PROJECTS[Math.floor(r() * PROJECTS.length)], today: isToday });
      }
    }
  }
  const sumW = (t: boolean) => raw.filter((x) => x.today === t).reduce((s, x) => s + x.w, 0) || 1;
  const kPast = pastAmt / sumW(false), kToday = todayAmt / sumW(true);
  const records: UsageRecord[] = raw.map((x, n) => {
    const aic = x.w * (x.today ? kToday : kPast);
    const tok = Math.max(aic, 0.002) / x.model.rate * 1e6;
    const input = Math.round(tok * 0.97);
    return {
      id: `studio:${n}`, ts: Math.min(x.ts, now - 60_000), client: CLIENT_OF[x.project], model: x.model.id,
      tokens: { input, output: Math.round(tok * 0.03), cacheRead: Math.round(input * 0.8), cacheWrite: Math.round(input * 0.08), reasoning: Math.round(tok * 0.004) },
      aic, requests: Math.max(1, Math.round(tok / 42000)), project: x.project, session: demoSession(x.project, x.ts),
      initiator: r() < 0.7 ? "agent" : "user", reasoningEffort: ["low", "medium", "high"][Math.floor(r() * 3)], isAuto: r() < 0.4,
    };
  });
  const connected = o.connection === "ok" || o.connection === "stale";
  const entitlement = o.allowance > 0 ? o.allowance : null;
  const reported = connected ? o.used : null;
  const githubPlan: GitHubPlan | null = connected ? { login: "octocat", plan: "individual", sku: "copilot_pro", creditsUsed: o.used, entitlement: entitlement, overagePermitted: o.overagePermitted, unlimited: false, resetAt: cycle.end, fetchedAt: now } : null;
  const { allowance, ...source } = allowanceOf(settings.allowanceOverride, githubPlan);
  const pace = computePace(records, cycle, allowance, reported, now, settings.paceTolerancePct / 100);
  return {
    records, now, pace, settings, ...source, githubPlan, ...overageOf(githubPlan, pace.used),
    status: {
      lastRefresh: o.connection === "stale" ? now - 3 * 3_600_000 : connected ? now - 40_000 : null,
      planError: o.connection === "offline" || o.connection === "stale" ? { message: "Can't reach GitHub: check your internet connection", offline: true } : null,
      signIn: o.connection === "offline" || o.connection === "stale" ? { state: "error", message: "Can't reach GitHub: check your internet connection" } : connected ? { state: "signedIn", login: "octocat" } : { state: "unknown" },
      cliFiles: 23, connectingSince: o.connection === "connecting" ? now - 3000 : null,
    },
  };
}
