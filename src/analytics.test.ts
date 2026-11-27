import { describe, expect, it } from "vitest";
import { addUtcMonths, anchorDayKey, breakdown, CLIENT_NAMES, clientsIn, cycleDayTotals, cycleDays, dailyStacks, DAY, dayKey, labelFor, modelName, previousCycle, projectShares, projectTotals, rangeDays, rangeInterval, rangeOrDefault, recordsFor, sessionSummaries, total, UNLOGGED, unloggedIn, usageCsv, withUnlogged } from "./analytics";
import { buildDemo, computePace, DEFAULT_SETTINGS, demoSnapshot } from "./dev/demo";
import * as f from "./format";
import type { UsageRecord } from "./types";

const rec = (id: string, iso: string, client: UsageRecord["client"], model: string, aic: number | null, input = 0, output = 0): UsageRecord => ({
  id, ts: new Date(iso).getTime(), client, model, tokens: { input, output, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
  aic, requests: 1, project: null, session: null, initiator: null, reasoningEffort: null, isAuto: null,
});

describe("credits come from local logs", () => {
  it("tokens and credits both come from the same local records", () => {
    const rs = [
      rec("1", "2026-11-01T10:00:00Z", "cli", "a", 1, 100),
      rec("3", "2026-11-02T10:00:00Z", "cli", "a", 2, 50),
      rec("4", "2026-11-02T11:00:00Z", "cli", "b", null, 70, 5),
    ];
    expect(total(rs, "aic")).toBe(3);
    expect(total(rs, "inputTokens")).toBe(220);
    expect(recordsFor(rs, "requests").length).toBe(3);
  });
});

describe("Unlogged Usage slice", () => {
  const rs = [rec("1", "2026-11-03T10:00:00Z", "cli", "a", 10), rec("2", "2026-11-04T10:00:00Z", "cli", "b", 5)];

  it("adds a Not in local logs row so the breakdown adds up to Used", () => {
    const rows = withUnlogged(breakdown(rs, "aic", "model"), 25);
    expect(rows.map((r) => [r.key, r.value, r.share])).toEqual([["a", 10, 0.25], ["b", 5, 0.125], [UNLOGGED, 25, 0.625]]);
    expect(rows.reduce((s, r) => s + r.value, 0)).toBe(40);
  });

  it("leaves the breakdown alone when the logs cover Used", () => {
    const rows = breakdown(rs, "aic", "model");
    expect(withUnlogged(rows, 0)).toEqual(rows);
  });

  it("is all Unlogged Usage when nothing is logged", () => {
    expect(withUnlogged([], 12)).toEqual([{ key: UNLOGGED, value: 12, share: 1 }]);
  });

  it("only applies to ranges that start with the Cycle", () => {
    const snap = demoSnapshot("ahead");
    snap.pace.unlogged = 7;
    const { start, end } = snap.pace.cycle;
    expect(unloggedIn(snap, { start, end })).toBe(7);
    expect(unloggedIn(snap, { start: start + DAY, end })).toBe(0);
  });

  it("belongs to no Client, so a Client filter that narrows the view leaves it out", () => {
    const snap = demoSnapshot("ahead");
    snap.pace.unlogged = 7;
    const iv = snap.pace.cycle;
    const present = clientsIn(snap.records);
    expect(present.length).toBeGreaterThan(1);
    expect(unloggedIn(snap, iv, new Set(present))).toBe(7);
    expect(unloggedIn(snap, iv, new Set(present.slice(1)))).toBe(0);
  });
});

describe("aggregations", () => {
  const rs = [
    rec("1", "2026-11-01T10:00:00", "cli", "a", 1),
    rec("2", "2026-11-01T11:00:00", "cli", "b", 3),
    rec("3", "2026-11-03T10:00:00", "cli", "a", 2),
  ];
  const iv = { start: new Date("2026-11-01T00:00:00").getTime(), end: new Date("2026-11-04T00:00:00").getTime() };

  it("zero-fills daily stacks and folds extra groups into Other", () => {
    const { days, groups } = dailyStacks(rs, "aic", iv, "model", 1);
    expect(days.map((d) => d.total)).toEqual([4, 0, 2]);
    expect(groups).toEqual(["a", "Other"]);
    expect(days[0].parts).toEqual([{ group: "a", value: 1 }, { group: "Other", value: 3 }]);
  });

  it("breaks down by model, tied models included", () => {
    const b = breakdown(rs, "aic", "model");
    expect(new Set(b.map((r) => r.key))).toEqual(new Set(["a", "b"]));
    expect(b.reduce((s, r) => s + r.share, 0)).toBeCloseTo(1);
  });

});

describe("pace (demo mirror of the Rust logic)", () => {
  const cycle = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
  it("ahead of pace projects a run-out date", () => {
    const rs = Array.from({ length: 15 }, (_, i) => rec(String(i), new Date(Date.UTC(2026, 10, i + 1, 10)).toISOString(), "cli", "m", 40));
    const p = computePace(rs, cycle, 1000, null, Date.UTC(2026, 10, 16), 0.05);
    expect(p.status).toBe("ahead");
    expect(p.used).toBe(600);
    expect(p.projected).toBe(1200);
    expect(p.runOutAt).toBe(Date.UTC(2026, 10, 26));
  });
  it("every demo scenario lands on its status", () => {
    expect(demoSnapshot("under").pace.status).toBe("under");
    expect(demoSnapshot("onpace").pace.status).toBe("onPace");
    expect(demoSnapshot("ahead").pace.status).toBe("ahead");
    expect(demoSnapshot("exhausted").pace.status).toBe("exhausted");
  });
  it("demo usage comes only from session-state logs, which always report AIC", () => {
    const { records } = demoSnapshot("onpace");
    expect(records.every((r) => r.aic != null)).toBe(true);
  });
  it("demo usage spans all three Clients, one Client per Session", () => {
    const { records } = demoSnapshot("onpace");
    expect(clientsIn(records)).toEqual(["cli", "vscode", "app"]);
    const bySession = new Map<string, Set<string>>();
    for (const r of records) bySession.set(r.session!.id, (bySession.get(r.session!.id) ?? new Set()).add(r.client));
    expect([...bySession.values()].every((s) => s.size === 1)).toBe(true);
  });
});

describe("formatting", () => {
  it("formats AIC, tokens and model names", () => {
    expect(f.aic(0)).toBe("0");
    expect(f.aic(1.234)).toBe(new Intl.NumberFormat(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(1.23));
    expect(f.aic(206.4)).toBe("206");
    expect(f.tokens(1_234_567)).toBe(`${new Intl.NumberFormat(undefined, { minimumFractionDigits: 1 }).format(1.2)}M`);
    expect(modelName("gpt-5.6-luna")).toBe("GPT-5.6 Luna");
    expect(modelName("claude-sonnet-4.6")).toBe("Claude Sonnet 4.6");
  });
});

describe("stored dashboard range", () => {
  it("keeps a known range key", () => {
    expect(rangeOrDefault("last30")).toBe("last30");
    expect(rangeOrDefault("previousCycle")).toBe("previousCycle");
  });
  it("falls back to This Cycle for a missing or unknown key", () => {
    expect(rangeOrDefault(null)).toBe("cycle");
    expect(rangeOrDefault("bogus")).toBe("cycle");
    expect(rangeOrDefault("Last 7 days")).toBe("cycle");
  });
});

describe("cycleDays", () => {
  it("returns exactly the cycle's days in the local timezone", async () => {
    const { cycleDays } = await import("./analytics");
    const days = cycleDays({ start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) });
    expect(days.length).toBe(30);
    expect(new Date(days[0]).getDate()).toBe(1);
    expect(new Date(days[29]).getDate()).toBe(30);
  });
});

describe("weekLayout: how many week rows a month's calendar needs", () => {
  const month = async (y: number, m: number) => (await import("./analytics")).cycleDays({ start: new Date(y, m, 1).getTime(), end: new Date(y, m + 1, 1).getTime() });
  const layout = async (y: number, m: number, mondayFirst: boolean) => (await import("./analytics")).weekLayout(await month(y, m), mondayFirst);

  it("Nov 2026 starts on a Sunday: six rows when weeks start on Monday, five when they start on Sunday", async () => {
    expect(await layout(2026, 10, true)).toEqual({ lead: 6, rows: 6 });
    expect(await layout(2026, 10, false)).toEqual({ lead: 0, rows: 5 });
  });

  it("a month starting on the first weekday needs no padding", async () => {
    expect(await layout(2027, 1, true)).toEqual({ lead: 0, rows: 4 });
    expect(await layout(2026, 1, false)).toEqual({ lead: 0, rows: 4 });
    expect(await layout(2026, 5, true)).toEqual({ lead: 0, rows: 5 });
  });

  it("Aug 2026 (31 days from a Saturday) needs six rows either way", async () => {
    expect(await layout(2026, 7, true)).toEqual({ lead: 5, rows: 6 });
    expect(await layout(2026, 7, false)).toEqual({ lead: 6, rows: 6 });
  });
});

describe("Client filter", () => {
  it("2 AIC per 1M CLI tokens; credits only from records that log them", () => {
    const rs = [
      rec("c", "2026-11-05T09:00:00Z", "cli", "m", 2, 1_000_000, 0),
      rec("t", "2026-11-05T12:00:00Z", "cli", "m", null, 0, 0),
    ];
    expect(recordsFor(rs, "aic").length).toBe(1);
    expect(total(rs, "aic", undefined, new Set(["cli"]))).toBe(2);
    expect(total(rs, "aic", undefined, new Set(["cli"])) / (total(rs, "totalTokens", undefined, new Set(["cli"])) / 1e6)).toBe(2);
    expect(total(rs, "aic", undefined, new Set())).toBe(0);
  });
});

describe("Client", () => {
  const rs = [rec("1", "2026-11-05T09:00:00Z", "cli", "m", 2, 10, 1)];
  it("names each Client", () => {
    expect(CLIENT_NAMES.cli).toBe("Copilot CLI");
    expect(CLIENT_NAMES.vscode).toBe("VS Code");
    expect(CLIENT_NAMES.app).toBe("Copilot App");
    expect(rs[0].client).toBe("cli");
  });
  it("the Client filter lists every Client present, in a fixed order", () => {
    const mixed = [rec("a", "2026-11-05T09:00:00Z", "app", "m", 1), rec("c", "2026-11-05T10:00:00Z", "cli", "m", 1), rec("v", "2026-11-05T11:00:00Z", "vscode", "m", 1), rec("c2", "2026-11-05T12:00:00Z", "cli", "m", 1)];
    expect(clientsIn(mixed)).toEqual(["cli", "vscode", "app"]);
    expect(clientsIn(rs)).toEqual(["cli"]);
    expect(clientsIn([])).toEqual(["cli"]);
  });
  it("CSV export has a client column and honours the Client filter", () => {
    const iv = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
    const [head, row, ...rest] = usageCsv(rs, iv, new Set(["cli"])).trim().split("\n");
    const cols = head.split(",");
    expect(cols).toContain("client");
    expect(cols).not.toContain("source");
    expect(row.split(",")[cols.indexOf("client")]).toBe("Copilot CLI");
    expect(rest).toEqual([]);
    expect(usageCsv(rs, iv, new Set()).trim().split("\n")).toEqual([head]);
  });
});

describe("Project and Session", () => {
  const inSession = (r: UsageRecord, project: string | null, id: string, label: string | null = null): UsageRecord => ({ ...r, project, session: { id, label } });
  const rs = [
    inSession(rec("1", "2026-11-01T10:00:00", "cli", "a", 1), "atlas", "s1", "Fix login"),
    inSession(rec("2", "2026-11-01T10:20:00", "cli", "b", 2), "atlas", "s1", "Fix login"),
    inSession(rec("3", "2026-11-02T09:00:00", "cli", "a", 4), "atlas", "s2", "Fix login"),
    inSession(rec("4", "2026-11-02T11:00:00", "cli", "a", 8), "web", "s3"),
    inSession(rec("5", "2026-11-03T11:00:00", "cli", "a", 16), null, "s4", "Say hi"),
  ];


  it("breaks down by Project, summing that Project's Sessions", () => {
    const rows = breakdown(rs, "aic", "project");
    expect(rows.map((r) => [labelFor(r.key, "project"), r.value])).toEqual([["No project", 16], ["web", 8], ["atlas", 7]]);
  });

  it("breaks down by Session, keeping Sessions with the same label apart", () => {
    const rows = breakdown(rs, "aic", "session");
    expect(rows.map((r) => [labelFor(r.key, "session"), r.value])).toEqual([["Say hi", 16], ["s3", 8], ["Fix login", 4], ["Fix login", 3]]);
  });


  it("lists Sessions with their Project and Client, and totals per Project", () => {
    const iv = { start: new Date("2026-11-01T00:00:00").getTime(), end: new Date("2026-11-04T00:00:00").getTime() };
    const sessions = sessionSummaries(rs, iv, new Set(["cli"]));
    const s1 = sessions.find((s) => s.id === "s1")!;
    expect(s1).toMatchObject({ label: "Fix login", project: "atlas", client: "Copilot CLI", aic: 3, start: rs[0].ts });
    expect(sessions.find((s) => s.id === "s4")!.project).toBe("No project");
    const totals = projectTotals(sessions);
    expect(totals.map((p) => [p.project, p.aic, p.sessions])).toEqual([["No project", 16, 1], ["web", 8, 1], ["atlas", 7, 2]]);
  });

  it("gives Project shares of Used, with a Not in local logs row, so they add up", () => {
    const iv = { start: new Date("2026-11-01T00:00:00").getTime(), end: new Date("2026-11-04T00:00:00").getTime() };
    const rows = projectShares(projectTotals(sessionSummaries(rs, iv, new Set(["cli"]))), 9);
    expect(rows.map((r) => [r.key, r.value, r.share])).toEqual([["No project", 16, 0.4], ["web", 8, 0.2], ["atlas", 7, 0.175], [UNLOGGED, 9, 0.225]]);
    expect(projectShares([], 0)).toEqual([]);
  });

  it("flags a Runaway Session by its busiest hour", () => {
    const iv = { start: 0, end: Date.now() * 2 };
    const sessions = sessionSummaries(rs, iv, new Set(["cli"]));
    expect(sessions.find((s) => s.id === "s1")!.peak).toBe(3);
    expect(sessions.find((s) => s.id === "s2")!.peak).toBe(4);
  });
});

describe("Overage (demo mirror of the Rust logic)", () => {
  const studio = (used: number, overagePermitted: boolean, allowanceOverride = 0) =>
    buildDemo({ used, allowance: 300, day: 16, today: 0, localShare: 1, overagePermitted, connection: "ok" }, { ...DEFAULT_SETTINGS, allowanceOverride });

  it("not permitted and the Entitlement used up: Copilot has stopped", () => {
    const s = studio(300, false);
    expect([s.pace.status, s.entitlementUsedUp, s.copilotStopped, s.overage]).toEqual(["exhausted", true, true, 0]);
  });
  it("permitted: AIC above the Entitlement is Overage", () => {
    const s = studio(345, true);
    expect([s.pace.status, s.entitlementUsedUp, s.copilotStopped]).toEqual(["exhausted", true, false]);
    expect(s.overage).toBeCloseTo(45);
  });
  it("own Allowance below the Entitlement: Exhausted while Copilot still works", () => {
    const s = studio(150, false, 120);
    expect([s.pace.status, s.entitlementUsedUp, s.copilotStopped, s.overage]).toEqual(["exhausted", false, false, 0]);
  });
  it("own Allowance above the Entitlement, not permitted: capped at the Entitlement", () => {
    const s = studio(300, false, 500);
    expect([s.pace.allowance, s.allowanceCapped, s.pace.status, s.copilotStopped]).toEqual([300, true, "exhausted", true]);
    expect(s.settings.allowanceOverride).toBe(500);
  });
  it("own Allowance above the Entitlement, permitted: the Allowance stands", () => {
    const s = studio(300, true, 500);
    expect([s.pace.allowance, s.allowanceCapped]).toEqual([500, false]);
    expect(s.pace.status).not.toBe("exhausted");
  });
});

describe("usage CSV cells", () => {
  const iv = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
  const cli = new Set(["cli"]);
  const header = "timestamp,local_date,client,model,project,session,session_label,input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,reasoning_tokens,aic,requests,initiator,reasoning_effort,auto";

  it("names the AIC column aic and keeps the other columns", () => {
    expect(usageCsv([], iv, cli)).toBe(`${header}\n`);
  });

  it("guards formula-leading text and quotes embedded quotes, commas and CR", () => {
    const r = { ...rec("1", "2026-11-05T09:00:00Z", "cli", "m", 2, 10, 1), project: '=HYPERLINK("http://x","c")', session: { id: "s1", label: "@SUM(1+1)\r+cmd|x" } };
    expect(usageCsv([r], iv, cli)).toBe(`${header}\n` + [
      "2026-11-05T09:00:00.000Z", "2026-11-05", "Copilot CLI", "m", '"\'=HYPERLINK(""http://x"",""c"")"', "s1", '"\'@SUM(1+1)\r+cmd|x"', "10", "1", "0", "0", "0", "2", "1", "", "", "",
    ].join(",") + "\n");
  });

  it("guards each formula trigger character", () => {
    const labelled = (label: string) => ({ ...rec("1", "2026-11-05T09:00:00Z", "cli", "m", 2), session: { id: "s", label } });
    const cellsFor = (label: string) => usageCsv([labelled(label)], iv, cli).split("\n")[1].split(",")[6];
    expect(cellsFor("- fix the tests")).toBe("'- fix the tests");
    expect(cellsFor("+1 retry")).toBe("'+1 retry");
    expect(cellsFor("=1+1")).toBe("'=1+1");
    expect(cellsFor("@team review")).toBe("'@team review");
    expect(cellsFor("\tindented")).toBe("'\tindented");
    expect(cellsFor("plain text")).toBe("plain text");
  });

  it("quotes cells with quotes or commas and leaves numbers plain", () => {
    const r = { ...rec("1", "2026-11-05T09:00:00Z", "cli", "m", -3, 1234, 0), session: { id: "s", label: 'say "hi", ok' } };
    const row = usageCsv([r], iv, cli).split("\n")[1];
    expect(row).toContain(',"say ""hi"", ok",');
    expect(row).toContain(",1234,0,0,0,0,-3,1,");
  });
});

describe("previous Cycle", () => {
  const iso = (ts: number) => new Date(ts).toISOString();
  it("clamps a day that the previous month does not have", () => {
    expect(iso(addUtcMonths(Date.UTC(2026, 2, 31), -1))).toBe("2026-02-28T00:00:00.000Z");
    expect(iso(addUtcMonths(Date.UTC(2028, 2, 31), -1))).toBe("2028-02-29T00:00:00.000Z");
    expect(iso(addUtcMonths(Date.UTC(2026, 0, 31), -1))).toBe("2025-12-31T00:00:00.000Z");
    expect(iso(addUtcMonths(Date.UTC(2026, 9, 1), -1))).toBe("2026-09-01T00:00:00.000Z");
  });
  it("runs from the clamped start up to this Cycle's start", () => {
    const cycle = { start: Date.UTC(2026, 2, 31), end: Date.UTC(2026, 3, 30) };
    expect(previousCycle(cycle)).toEqual({ start: Date.UTC(2026, 1, 28), end: cycle.start });
  });
  it("is the Last Cycle range with the same clamped start", () => {
    const snap = demoSnapshot("ahead");
    snap.pace = { ...snap.pace, cycle: { start: Date.UTC(2026, 2, 31), end: Date.UTC(2026, 3, 30) } };
    expect(rangeInterval("previousCycle", snap)).toEqual({ start: Date.UTC(2026, 1, 28), end: Date.UTC(2026, 2, 31) });
  });
});

describe("Cycle day totals", () => {
  const cycle = { start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) };
  it("keeps records from the Cycle's first and last hours in the first and last day", () => {
    const rs = [
      rec("early", "2026-11-01T00:30:00Z", "cli", "m", 1),
      rec("mid", "2026-11-15T12:00:00Z", "cli", "m", 4),
      rec("late", "2026-11-30T23:30:00Z", "cli", "m", 2),
      rec("before", "2026-10-31T23:30:00Z", "cli", "m", 100),
      rec("after", "2026-12-01T00:30:00Z", "cli", "m", 100),
    ];
    const totals = cycleDayTotals(rs, cycle);
    const days = cycleDays(cycle).map(dayKey);
    expect([...totals.keys()].every((k) => days.includes(k))).toBe(true);
    expect(total(rs, "aic", cycle)).toBe(7);
    expect([...totals.values()].reduce((a, b) => a + b, 0)).toBe(7);
  });
  it("gives the Cycle's own days the Logged AIC on the Overview axis too", () => {
    const rs = [rec("early", "2026-11-01T00:30:00Z", "cli", "m", 1), rec("late", "2026-11-30T23:30:00Z", "cli", "m", 2)];
    const stacks = dailyStacks(rs, "aic", cycle, null, 6, undefined, rangeDays(cycle, cycle));
    expect(stacks.days.length).toBe(cycleDays(cycle).length);
    expect(stacks.days.reduce((s, d) => s + d.total, 0)).toBe(3);
  });
});

describe("calendar anchor", () => {
  const days = [{ key: "2026-11-01", ts: 1 }, { key: "2026-11-02", ts: 2 }, { key: "2026-11-03", ts: 3 }];
  it("selects today when today is one of the Cycle's days", () => {
    expect(anchorDayKey(days, "2026-11-02", 2)).toBe("2026-11-02");
  });
  it("selects the last elapsed day when today is outside the Cycle's days", () => {
    expect(anchorDayKey(days, "2026-10-31", 2.5)).toBe("2026-11-02");
  });
});

