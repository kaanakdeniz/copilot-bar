import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { cycleDayTotals, cycleDays, dailyStacks, dayKey, rangeDays, total } from "./analytics";
import { computePace } from "./dev/demo";
import type { UsageRecord } from "./types";

const runtime = (globalThis as unknown as { process: { env: { TZ?: string } } }).process;
const savedTz = runtime.env.TZ;
const cycle = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
const now = Date.UTC(2026, 10, 30, 23, 59, 59);

const at = (iso: string, aic: number): UsageRecord => ({
  id: iso, ts: Date.parse(iso), client: "cli", model: "m", tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
  aic, requests: 1, project: null, session: null, initiator: null, reasoningEffort: null, isAuto: null,
});

const records: UsageRecord[] = [
  at("2026-10-31T23:30:00Z", 100),
  at("2026-11-01T00:30:00Z", 1),
  at("2026-11-01T02:00:00Z", 2),
  at("2026-11-01T05:00:00Z", 3),
  at("2026-11-15T12:00:00Z", 4),
  at("2026-11-30T23:00:00Z", 5),
  at("2026-11-30T23:59:00Z", 6),
  at("2026-12-01T00:30:00Z", 100),
];
const LOGGED = 21;

const EDGES: Record<string, { first: [string, number]; last: [string, number] }> = {
  "America/Los_Angeles": { first: ["2026-11-01", 6], last: ["2026-11-30", 11] },
  "Europe/Istanbul": { first: ["2026-11-01", 6], last: ["2026-11-30", 11] },
  "Pacific/Auckland": { first: ["2026-11-02", 6], last: ["2026-12-01", 11] },
};

describe.each(Object.keys(EDGES))("Cycle days in %s", (zone) => {
  beforeAll(() => { runtime.env.TZ = zone; });
  afterAll(() => { runtime.env.TZ = savedTz; });

  it("adds every day total up to the Cycle's Logged AIC", () => {
    const totals = cycleDayTotals(records, cycle);
    const sum = [...totals.values()].reduce((a, b) => a + b, 0);
    expect(sum).toBe(LOGGED);
    expect(sum).toBe(total(records, "aic", cycle));
    expect(computePace(records, cycle, 300, null, now, 0.05).loggedAic).toBe(LOGGED);
  });

  it("puts the Cycle's partial first and last hours on the edge days", () => {
    const { first, last } = EDGES[zone];
    const totals = cycleDayTotals(records, cycle);
    const days = cycleDays(cycle).map(dayKey);
    expect(days[0]).toBe(first[0]);
    expect(days[days.length - 1]).toBe(last[0]);
    expect(totals.get(first[0])).toBe(first[1]);
    expect(totals.get(last[0])).toBe(last[1]);
  });

  it("keeps every total on a displayed day", () => {
    const days = new Set(cycleDays(cycle).map(dayKey));
    expect([...cycleDayTotals(records, cycle).keys()].every((k) => days.has(k))).toBe(true);
  });

  it("stacks the same days, so the bars add up to the Cycle's Logged AIC", () => {
    const stacks = dailyStacks(records, "aic", cycle, "model", 6, undefined, rangeDays(cycle, cycle));
    expect(stacks.days.length).toBe(cycleDays(cycle).length);
    expect(stacks.days.reduce((s, d) => s + d.total, 0)).toBe(LOGGED);
  });
});
