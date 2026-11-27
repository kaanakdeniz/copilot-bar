
import type { Client, RangeKey, Snapshot, UsageRecord } from "./types";

export const DAY = 86_400_000;

export type Metric = "aic" | "totalTokens" | "inputTokens" | "outputTokens" | "cacheReadTokens" | "cacheWriteTokens" | "reasoningTokens" | "requests";
export type Dim = "day" | "model" | "project" | "session";

export function value(r: UsageRecord, m: Metric): number {
  switch (m) {
    case "aic": return r.aic ?? 0;
    case "totalTokens": return r.tokens.input + r.tokens.output;
    case "inputTokens": return r.tokens.input;
    case "outputTokens": return r.tokens.output;
    case "cacheReadTokens": return r.tokens.cacheRead;
    case "cacheWriteTokens": return r.tokens.cacheWrite;
    case "reasoningTokens": return r.tokens.reasoning;
    case "requests": return r.requests;
  }
}

const pad = (n: number) => String(n).padStart(2, "0");
export const dayKey = (ts: number) => { const d = new Date(ts); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`; };
export const recDay = (r: UsageRecord) => dayKey(r.ts);
export function startOfDay(ts: number): number { const d = new Date(ts); d.setHours(0, 0, 0, 0); return d.getTime(); }
export function addDays(ts: number, n: number): number { const d = new Date(ts); d.setDate(d.getDate() + n); return d.getTime(); }
export function dayFromKey(k: string): number { const [y, m, d] = k.split("-").map(Number); return new Date(y, m - 1, d).getTime(); }

const SPECIAL: Record<string, string> = { gpt: "GPT", mini: "mini", nano: "nano", o1: "o1", o3: "o3", o4: "o4" };
export function modelName(model: string): string {
  return model.split("-").map((p) => {
    if (SPECIAL[p]) return SPECIAL[p];
    return /^[a-z]/.test(p) ? p[0].toUpperCase() + p.slice(1) : p;
  }).join(" ").replace(/^GPT (\d)/, "GPT-$1");
}

export const CLIENT_NAMES: Record<Client, string> = { cli: "Copilot CLI", vscode: "VS Code", app: "Copilot App" };

export function clientsIn(records: UsageRecord[]): Client[] {
  const seen = new Set(records.map((r) => r.client));
  const present = (Object.keys(CLIENT_NAMES) as Client[]).filter((c) => seen.has(c));
  return present.length ? present : ["cli"];
}

export function keyFor(r: UsageRecord, d: Dim): string {
  switch (d) {
    case "day": return recDay(r);
    case "model": return r.model;
    case "project": return projectOf(r);
    case "session": return sessionKey(r);
  }
}

export const NO_PROJECT = "No project";
export const projectOf = (r: UsageRecord) => r.project ?? NO_PROJECT;
const sessionKey = (r: UsageRecord) => (r.session ? `${r.session.id}|${r.session.label ?? r.session.id}` : "|Unknown");

export function labelFor(k: string, d: Dim): string {
  if (d === "session") return k.slice(k.indexOf("|") + 1);
  if (d === "model") return modelName(k);
  return k;
}

export const groupLabel = (k: string, d: Dim): string => (k === "Other" ? "Other" : labelFor(k, d));

export interface Interval { start: number; end: number }

export function recordsFor(records: UsageRecord[], m: Metric, iv?: Interval, clients?: Set<string>): UsageRecord[] {
  return records.filter((r) => {
    if (clients && !clients.has(r.client)) return false;
    if (iv && (r.ts < iv.start || r.ts >= iv.end)) return false;
    return m !== "aic" || r.aic != null;
  });
}

export const total = (records: UsageRecord[], m: Metric, iv?: Interval, clients?: Set<string>) =>
  recordsFor(records, m, iv, clients).reduce((s, r) => s + value(r, m), 0);

export interface Row { key: string; value: number; share: number }

export function breakdown(records: UsageRecord[], m: Metric, d: Dim, iv?: Interval, clients?: Set<string>): Row[] {
  const totals = new Map<string, number>();
  for (const r of recordsFor(records, m, iv, clients)) {
    const v = value(r, m);
    if (!v) continue;
    const k = keyFor(r, d);
    totals.set(k, (totals.get(k) ?? 0) + v);
  }
  const sum = [...totals.values()].reduce((a, b) => a + b, 0);
  const rows = [...totals].map(([key, v]) => ({ key, value: v, share: sum ? v / sum : 0 }));
  return d === "day" ? rows.sort((a, b) => a.key.localeCompare(b.key)) : rows.sort((a, b) => b.value - a.value);
}

export const UNLOGGED = "Not in local logs";

export const UNLOGGED_MIN_AIC = 0.5;

export function unloggedIn(snap: Snapshot, iv: Interval, clients?: Set<string>): number {
  if (iv.start !== snap.pace.cycle.start) return 0;
  if (clients && clientsIn(snap.records).some((c) => !clients.has(c))) return 0;
  return snap.pace.unlogged;
}

export function withUnlogged(rows: Row[], unlogged: number): Row[] {
  if (!(unlogged > 0)) return rows;
  const sum = rows.reduce((s, r) => s + r.value, 0) + unlogged;
  return [...rows.map((r) => ({ ...r, share: r.value / sum })), { key: UNLOGGED, value: unlogged, share: unlogged / sum }];
}

export interface DayStack { day: number; key: string; total: number; parts: { group: string; value: number }[] }

const TOTAL_GROUP = "Total";
const OTHER_GROUP = "Other";

export function localDays(iv: Interval): number[] {
  const out: number[] = [];
  for (let t = startOfDay(iv.start); t < iv.end; t = addDays(t, 1)) out.push(t);
  return out;
}

export function displayDayKey(ts: number, days: number[]): string {
  if (!days.length) return dayKey(ts);
  if (ts < days[0]) return dayKey(days[0]);
  const last = days[days.length - 1];
  return ts >= addDays(last, 1) ? dayKey(last) : dayKey(ts);
}

export function dailyStacks(records: UsageRecord[], m: Metric, iv: Interval, d: Dim | null, topN = 6, clients?: Set<string>, days: number[] = localDays(iv)): { days: DayStack[]; groups: string[] } {
  const tagged = recordsFor(records, m, iv, clients).map((r) => ({ r, key: d ? keyFor(r, d) : TOTAL_GROUP }));
  const totals = new Map<string, number>();
  for (const { r, key } of tagged) totals.set(key, (totals.get(key) ?? 0) + value(r, m));
  const ranked = d ? [...totals].filter(([, v]) => v > 0).sort((a, b) => b[1] - a[1]).map(([k]) => k) : [TOTAL_GROUP];
  const groups = ranked.slice(0, topN);
  if (ranked.length > topN) groups.push(OTHER_GROUP);
  const kept = new Set(groups);
  const byDay = new Map<string, Map<string, number>>();
  for (const { r, key } of tagged) {
    const dk = displayDayKey(r.ts, days);
    const g = kept.has(key) ? key : OTHER_GROUP;
    const mm = byDay.get(dk) ?? new Map<string, number>();
    mm.set(g, (mm.get(g) ?? 0) + value(r, m));
    byDay.set(dk, mm);
  }
  const stacks = days.map((t): DayStack => {
    const k = dayKey(t);
    const mm = byDay.get(k);
    const parts = groups.map((g) => ({ group: g, value: mm?.get(g) ?? 0 }));
    return { day: t, key: k, total: parts.reduce((s, p) => s + p.value, 0), parts };
  });
  return { days: stacks, groups };
}

export function cycleDayTotals(records: UsageRecord[], cycle: Interval): Map<string, number> {
  const days = cycleDays(cycle);
  const totals = new Map<string, number>();
  for (const r of recordsFor(records, "aic", cycle)) {
    const k = displayDayKey(r.ts, days);
    totals.set(k, (totals.get(k) ?? 0) + (r.aic ?? 0));
  }
  return totals;
}

export function anchorDayKey(days: { key: string; ts: number }[], todayKey: string, now: number): string | undefined {
  if (days.some((d) => d.key === todayKey)) return todayKey;
  const elapsed = days.filter((d) => d.ts <= now);
  return (elapsed[elapsed.length - 1] ?? days[0])?.key;
}

export function rangeDays(iv: Interval, cycle: Interval): number[] {
  return iv.start === cycle.start ? cycleDays(cycle).filter((d) => d < iv.end) : localDays(iv);
}

export function addUtcMonths(ts: number, months: number): number {
  const d = new Date(ts);
  const target = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + months, 1, d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds(), d.getUTCMilliseconds()));
  const lastDay = new Date(Date.UTC(target.getUTCFullYear(), target.getUTCMonth() + 1, 0)).getUTCDate();
  target.setUTCDate(Math.min(d.getUTCDate(), lastDay));
  return target.getTime();
}

export function previousCycle(cycle: Interval): Interval {
  return { start: addUtcMonths(cycle.start, -1), end: cycle.start };
}

export function rangeInterval(range: RangeKey, snap: Snapshot): Interval {
  const now = snap.now;
  const tomorrow = addDays(startOfDay(now), 1);
  const lastDays = (n: number) => ({ start: addDays(startOfDay(now), -(n - 1)), end: tomorrow });
  const { start, end } = snap.pace.cycle;
  switch (range) {
    case "last7": return lastDays(7);
    case "last30": return lastDays(30);
    case "last90": return lastDays(90);
    case "cycle": return { start, end: Math.min(end, tomorrow) };
    case "previousCycle": return previousCycle({ start, end });
    case "all": {
      const first = snap.records.reduce((m, r) => Math.min(m, r.ts), now);
      return { start: startOfDay(first), end: tomorrow };
    }
  }
}

export const RANGES: { key: RangeKey; title: string }[] = [
  { key: "last7", title: "Last 7 days" },
  { key: "last30", title: "Last 30 days" },
  { key: "cycle", title: "This Cycle" },
  { key: "previousCycle", title: "Last Cycle" },
  { key: "last90", title: "Last 90 days" },
  { key: "all", title: "All time" },
];

export function rangeOrDefault(key: string | null): RangeKey {
  return RANGES.some((r) => r.key === key) ? (key as RangeKey) : "cycle";
}

export function cycleDays(cycle: { start: number; end: number }): number[] {
  const out: number[] = [];
  for (let t = addDays(startOfDay(cycle.start), -1); t < cycle.end + DAY; t = addDays(t, 1)) {
    const mid = t + DAY / 2;
    if (mid >= cycle.start && mid < cycle.end) out.push(t);
  }
  return out;
}

export function weekLayout(days: number[], weekStartsMonday: boolean): { lead: number; rows: number } {
  if (!days.length) return { lead: 0, rows: 0 };
  const first = new Date(days[0]).getDay();
  const lead = weekStartsMonday ? (first + 6) % 7 : first;
  return { lead, rows: Math.ceil((lead + days.length) / 7) };
}

export const todayInterval = (now: number): Interval => ({ start: startOfDay(now), end: addDays(startOfDay(now), 1) });
export const yesterdayInterval = (now: number): Interval => ({ start: addDays(startOfDay(now), -1), end: startOfDay(now) });

export interface SessionSummary {
  id: string;
  label: string | null;
  project: string;
  client: string;
  start: number;
  aic: number;
  hasAic: boolean;
  tokens: number;
  models: Map<string, number>;
  peak: number;
}

function peakHour(calls: [number, number][]): number {
  calls.sort((a, b) => a[0] - b[0]);
  let best = 0, sum = 0, lo = 0;
  for (let hi = 0; hi < calls.length; hi++) {
    sum += calls[hi][1];
    while (calls[hi][0] - calls[lo][0] > 3_600_000) sum -= calls[lo++][1];
    best = Math.max(best, sum);
  }
  return best;
}

export function sessionSummaries(records: UsageRecord[], iv: Interval, clients: Set<string>): SessionSummary[] {
  type Acc = SessionSummary & { calls: [number, number][] };
  const m = new Map<string, Acc>();
  for (const r of records) {
    if (!r.session || r.ts < iv.start || r.ts >= iv.end || !clients.has(r.client)) continue;
    const key = r.client + ":" + r.session.id;
    const s: Acc = m.get(key) ?? {
      id: r.session.id, label: r.session.label, project: projectOf(r), client: CLIENT_NAMES[r.client] ?? r.client,
      start: r.ts, aic: 0, hasAic: false, tokens: 0, models: new Map(), peak: 0, calls: [],
    };
    s.start = Math.min(s.start, r.ts);
    if (r.aic != null) { s.aic += r.aic; s.hasAic = true; s.calls.push([r.ts, r.aic]); }
    s.tokens += r.tokens.input + r.tokens.output;
    s.models.set(r.model, (s.models.get(r.model) ?? 0) + (r.aic ?? 0) * 1e6 + r.tokens.input + r.tokens.output);
    m.set(key, s);
  }
  return [...m.values()].map(({ calls, ...s }) => ({ ...s, peak: peakHour(calls) }));
}

export interface ProjectTotal { project: string; aic: number; tokens: number; sessions: number }

export function projectTotals(sessions: SessionSummary[]): ProjectTotal[] {
  const m = new Map<string, ProjectTotal>();
  for (const s of sessions) {
    const p = m.get(s.project) ?? { project: s.project, aic: 0, tokens: 0, sessions: 0 };
    p.aic += s.aic; p.tokens += s.tokens; p.sessions += 1;
    m.set(s.project, p);
  }
  return [...m.values()].sort((a, b) => b.aic - a.aic || b.tokens - a.tokens);
}

export function projectShares(projects: ProjectTotal[], unlogged: number): Row[] {
  const sum = projects.reduce((s, p) => s + p.aic, 0);
  return withUnlogged(projects.map((p) => ({ key: p.project, value: p.aic, share: sum ? p.aic / sum : 0 })), unlogged);
}

const FORMULA_TRIGGER = /^[=+\-@\t\r]/;
const QUOTE_TRIGGER = /[",\r\n]/;

function csvCell(v: unknown): string {
  if (v == null) return "";
  if (typeof v === "number") return String(v);
  const text = String(v);
  const guarded = FORMULA_TRIGGER.test(text) ? `'${text}` : text;
  return QUOTE_TRIGGER.test(guarded) ? `"${guarded.replace(/"/g, '""')}"` : guarded;
}

export function usageCsv(records: UsageRecord[], iv: Interval, clients: Set<string>): string {
  const head = ["timestamp", "local_date", "client", "model", "project", "session", "session_label", "input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens", "reasoning_tokens", "aic", "requests", "initiator", "reasoning_effort", "auto"];
  const rows = records
    .filter((r) => r.ts >= iv.start && r.ts < iv.end && clients.has(r.client))
    .sort((a, b) => a.ts - b.ts)
    .map((r) => [new Date(r.ts).toISOString(), recDay(r), CLIENT_NAMES[r.client] ?? r.client, r.model, r.project, r.session?.id, r.session?.label, r.tokens.input, r.tokens.output, r.tokens.cacheRead, r.tokens.cacheWrite, r.tokens.reasoning, r.aic, r.requests, r.initiator, r.reasoningEffort, r.isAuto].map(csvCell).join(","));
  return [head.join(","), ...rows].join("\n") + "\n";
}
