import type React from "react";
import { useEffect, useMemo, useState } from "react";
import {
  addDays, breakdown, CLIENT_NAMES, clientsIn, cycleDayTotals, cycleDays, DAY, dailyStacks, dayFromKey, dayKey, groupLabel, modelName, previousCycle, projectShares, projectTotals, rangeDays, rangeInterval, rangeOrDefault, RANGES, recordsFor, sessionSummaries, startOfDay, total, UNLOGGED, UNLOGGED_MIN_AIC, unloggedIn, usageCsv,
  type Dim, type Interval, type Metric,
} from "./analytics";
import { api } from "./backend";
import { Amount, CAPPED_NOTE, colorMap, CopilotIcon, Icon, Seg, StackedBars, StatusChip, UNLOGGED_NOTE, useTip, useWidth } from "./components";
import * as f from "./format";
import SettingsPage from "./SettingsPage";
import { messageOf } from "./settingsModel";
import { useSnapshot } from "./state";
import type { Client, Pace, RangeKey, Snapshot, Tab } from "./types";

const TABS: { key: Tab; title: string; icon: () => React.JSX.Element }[] = [
  { key: "overview", title: "Overview", icon: Icon.gauge },
  { key: "models", title: "Models", icon: Icon.cpu },
  { key: "projects", title: "Projects", icon: Icon.list },
  { key: "settings", title: "Settings", icon: Icon.gear },
];
const toTab = (t: string | null): Tab => TABS.find((x) => x.key === t)?.key ?? "overview";

const RANGE_KEY = "copilotbar.dashboard.range";
const recallRange = (): RangeKey => {
  try { return rangeOrDefault(localStorage.getItem(RANGE_KEY)); } catch { return "cycle"; }
};

export default function Dashboard() {
  const { snap, busy, refresh, update, refreshFailed, loadFailed, retry } = useSnapshot();
  const [tab, setTab] = useState<Tab>(() => toTab(new URLSearchParams(location.search).get("tab")));
  const [range, setRangeState] = useState<RangeKey>(recallRange);
  const setRange = (r: RangeKey) => { setRangeState(r); try { localStorage.setItem(RANGE_KEY, r); } catch {} };
  const [hidden, setHidden] = useState<Set<Client>>(new Set());
  const [exportNote, setExportNote] = useState<string | null>(null);
  useEffect(() => api.on("open-tab", (t) => setTab(toTab(t))), []);
  const present = useMemo(() => (snap ? clientsIn(snap.records) : []), [snap]);
  const clients = useMemo(() => {
    const shown = present.filter((c) => !hidden.has(c));
    return new Set<Client>(shown.length ? shown : present);
  }, [present, hidden]);
  const iv = useMemo(() => (snap ? rangeInterval(range, snap) : null), [snap, range]);
  if (!snap || !iv) {
    return loadFailed ? (
      <div className="empty" role="alert"><p>Couldn't read usage.</p><button className="btn" onClick={retry}>Retry</button></div>
    ) : <div className="empty">Loading…</div>;
  }
  const data = tab !== "settings";
  const saveCsv = async () => {
    setExportNote(null);
    try {
      const name = await exportCsv(snap, iv, clients, range);
      setExportNote(`Saved to Downloads/${name}`);
    } catch (e) {
      setExportNote(`Couldn't export: ${messageOf(e)}`);
    }
  };
  return (
    <div className="dash">
      <nav className="sidebar">
        <div className="brand"><CopilotIcon size={18} /> CopilotBar</div>
        <div role="tablist" aria-orientation="vertical" style={{ display: "contents" }}>
          {TABS.map((t) => (
            <button key={t.key} role="tab" aria-selected={tab === t.key} className={`tab ${tab === t.key ? "on" : ""}`} onClick={() => setTab(t.key)}>
              <t.icon /> {t.title}
            </button>
          ))}
        </div>
        <div className="foot">
          {snap.githubPlan ? <>@{snap.githubPlan.login}<br /></> : null}
          Updated {f.ago(snap.status.lastRefresh, snap.now)}
        </div>
      </nav>
      <main className="main">
        <div className="toolbar">
          <h1>{TABS.find((t) => t.key === tab)!.title}</h1>
          <span className="grow" />
          {data && (
            <>
              <ClientFilter present={present} clients={clients} setHidden={(on) => setHidden(new Set(present.filter((c) => !on.has(c))))} />
              <select className="field" value={range} onChange={(e) => setRange(e.target.value as RangeKey)} aria-label="Date range">
                {RANGES.map((x) => <option key={x.key} value={x.key}>{x.title}</option>)}
              </select>
              <button className="btn" onClick={() => void saveCsv()} title="Save as CSV to Downloads">
                <span className="row" style={{ gap: 6 }}><span style={{ width: 13, height: 13, display: "inline-block" }}><Icon.download /></span>Export CSV</span>
              </button>
              <button className="btn" onClick={refresh} disabled={busy}><span className="row" style={{ gap: 6 }}><span style={{ width: 13, height: 13, display: "inline-block" }}><Icon.refresh spinning={busy} /></span>Refresh</span></button>
            </>
          )}
        </div>
        {snap.status.planError && data && (
          <div className="banner"><span>⚠︎</span><span className="grow" style={{ userSelect: "text" }}>{snap.status.planError.message}</span><button className="link" onClick={() => setTab("settings")}>Fix in Settings</button></div>
        )}
        {refreshFailed && data && <div className="banner" role="alert"><span>⚠︎</span><span className="grow">Couldn't refresh. Try again.</span></div>}
        {exportNote && data && <p className="help" role="status" style={{ userSelect: "text" }}>{exportNote}</p>}
        <div key={tab} className="view-in">
          {tab === "overview" && <Overview snap={snap} iv={iv} clients={clients} rangeTitle={RANGES.find((x) => x.key === range)!.title} />}
          {tab === "models" && <Models snap={snap} iv={iv} clients={clients} />}
          {tab === "projects" && <Projects snap={snap} iv={iv} clients={clients} />}
          {tab === "settings" && <SettingsPage snap={snap} update={update} refresh={refresh} busy={busy} refreshFailed={refreshFailed} />}
        </div>
      </main>
    </div>
  );
}

function ClientFilter({ present, clients, setHidden }: { present: Client[]; clients: Set<Client>; setHidden: (shown: Set<Client>) => void }) {
  const all = present.map((k) => ({ k, t: CLIENT_NAMES[k] }));
  return (
    <span className="seg" role="group" aria-label="Client">
      {all.map((s) => {
        const on = clients.has(s.k);
        return (
          <button key={s.k} className={on ? "on" : "off"} aria-pressed={on}
            title={on ? `Hide ${s.t}` : `Show ${s.t}`} onClick={() => {
              const n = new Set(clients);
              n.has(s.k) ? n.delete(s.k) : n.add(s.k);
              if (n.size) setHidden(n);
            }}>{on ? "✓ " : ""}{s.t}</button>
        );
      })}
    </span>
  );
}

type P = { snap: Snapshot; iv: Interval; clients: Set<Client> };


function Overview({ snap, iv, clients, rangeTitle }: P & { rangeTitle: string }) {
  const color = useMemo(() => colorMap(snap), [snap]);
  const p = snap.pace;
  const [metric, setMetric] = useState<Metric>("aic");
  const [sel, setSel] = useState<string | null>(null);
  const figures = useMemo(() => ({
    unlogged: unloggedIn(snap, iv, clients),
    aic: total(snap.records, "aic", iv, clients),
    tokens: total(snap.records, "totalTokens", iv, clients),
    output: total(snap.records, "outputTokens", iv, clients),
    requests: total(snap.records, "requests", iv, clients),
    activeDays: new Set(recordsFor(snap.records, "requests", iv, clients).map((r) => dayKey(r.ts))).size,
  }), [snap, iv, clients]);
  const stacks = useMemo(() => dailyStacks(snap.records, metric, iv, "model", 6, clients, rangeDays(iv, p.cycle)), [snap, metric, iv, clients, p.cycle]);
  const { unlogged, aic, tokens, output, requests, activeDays } = figures;
  const days = Math.max(1, Math.round((Math.min(iv.end, addDays(startOfDay(snap.now), 1)) - iv.start) / DAY));
  const over = p.allowance != null && p.projected > p.allowance;
  const pick = (k: string) => {
    setSel(sel === k ? null : k);
    requestAnimationFrame(() => document.getElementById(`day-${k}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  };
  return (
    <div className="grid">
      <div className="card">
        <div className="row wrap" style={{ alignItems: "baseline", gap: 5, marginBottom: 6 }}>
          <Amount big={f.aic(p.used)} total={p.allowance != null ? `/ ${f.aic(p.allowance)}` : undefined} size={26} />
          <span className="muted" title={snap.allowanceCapped ? CAPPED_NOTE : undefined}>AIC this Cycle{snap.allowanceCapped ? " · capped at your Entitlement" : ""} · resets {f.resetDay(p.cycle.end)}</span>
          <span className="grow" />
          {p.status !== "noAllowance" && <StatusChip snap={snap} />}
        </div>
        <PaceChart snap={snap} />
        <div className="tiles" style={{ marginTop: 12, gridTemplateColumns: "repeat(4, minmax(0, 1fr))" }}>
          <Mini l="Avg / day" v={f.aic(p.avgDaily)} />
          <Mini l="Last 7 days / day" v={f.aic(p.avgRecent)} />
          <Mini l="Projected" v={f.aic(p.projected)} c={over ? "var(--orange-text)" : undefined} />
          <Mini l="Daily Budget" v={p.dailyBudget != null && p.status !== "exhausted" ? f.aic(p.dailyBudget) : "–"} />
        </div>
      </div>
      <div className="tiles" style={{ gridTemplateColumns: "repeat(3, minmax(0, 1fr))" }}>
        <Tile l={`AIC · ${rangeTitle.toLowerCase()}`} v={f.aic(aic + unlogged)}
          d={unlogged > 0 ? `${f.aic(aic)} in local logs · ${f.aic(unlogged)} not` : `${f.aic(aic / days)} per day`} />
        <Tile l="Tokens (in + out)" v={f.tokens(tokens)} d={`${f.tokens(output)} output`} />
        <Tile l="Requests" v={f.count(requests)} d={f.plural(activeDays, "active day")} />
      </div>
      <div className="card">
        <div className="row wrap" style={{ marginBottom: 8 }}>
          <b>{metric === "aic" ? "AIC" : "Tokens"} per day</b><span className="grow" />
          <span className="label">by model · click a day</span>
          <Seg label="Metric" value={metric} options={[["aic", "AIC"], ["totalTokens", "Tokens"]]} onChange={setMetric} />
        </div>
        <StackedBars days={stacks.days} groups={stacks.groups} metric={metric} color={color} height={200} width={900} dim="model"
          evenPerDay={metric === "aic" && p.allowance ? p.allowance / p.totalDays : null} evenLabel={false} todayKey={dayKey(snap.now)} onDay={pick} selected={sel ?? undefined} />
        <Legend groups={stacks.groups} color={color}>
          {metric === "aic" && p.allowance ? <LineKey color="var(--muted)" dash="4 3" label={`Even Pace ${f.short(p.allowance / p.totalDays)}/day`} /> : null}
        </Legend>
      </div>
      <DayTable snap={snap} iv={iv} clients={clients} sel={sel} onSel={setSel} />
    </div>
  );
}

function DayTable({ snap, iv, clients, sel, onSel }: P & { sel: string | null; onSel: (k: string | null) => void }) {
  const rows = useMemo(() => {
    const by = (m: Metric) => new Map(breakdown(snap.records, m, "day", iv, clients).map((r) => [r.key, r.value]));
    const aic = by("aic"), tok = by("totalTokens"), inp = by("inputTokens"), cr = by("cacheReadTokens"), rq = by("requests");
    const keys = [...new Set([...aic.keys(), ...tok.keys(), ...rq.keys()])].sort().reverse();
    return keys.map((k) => ({ k, aic: aic.get(k) ?? null, tok: tok.get(k) ?? 0, inp: inp.get(k) ?? 0, cr: cr.get(k) ?? 0, rq: rq.get(k) ?? 0 }));
  }, [snap, iv, clients]);
  const sum = rows.reduce((a, r) => ({ aic: a.aic + (r.aic ?? 0), tok: a.tok + r.tok, inp: a.inp + r.inp, cr: a.cr + r.cr, rq: a.rq + r.rq }), { aic: 0, tok: 0, inp: 0, cr: 0, rq: 0 });
  const cached = (cr: number, inp: number) => (inp > 0 ? f.pct(Math.min(1, cr / inp)) : "–");
  const gap = unloggedIn(snap, iv, clients);
  return (
    <div className="card" style={{ padding: 0, overflow: "auto" }}>
      <table className="data">
        <thead><tr><th>Day</th><th>AIC</th><th>Tokens</th><th title="Share of input tokens served from cache">Cached</th><th>Requests</th></tr></thead>
        <tbody>
          <tr className="total-row">
            <td>Total · {f.plural(rows.length, "day")}</td><td>{f.aic(sum.aic + gap)}</td><td>{f.tokens(sum.tok)}</td><td>{cached(sum.cr, sum.inp)}</td><td>{f.count(sum.rq)}</td>
          </tr>
          {gap > 0 && (
            <tr className="muted" title={UNLOGGED_NOTE}>
              <td>{UNLOGGED} (no date)</td><td>{f.aic(gap)}</td><td colSpan={3} />
            </tr>
          )}
          {rows.map((r) => (
            <tr key={r.k} id={`day-${r.k}`} onClick={() => onSel(sel === r.k ? null : r.k)} tabIndex={0} aria-selected={sel === r.k}
              onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSel(r.k)} style={{ background: sel === r.k ? "var(--accent-soft)" : undefined, cursor: "pointer" }}>
              <td>{f.weekdayDate(dayFromKey(r.k))}</td>
              <td title={r.aic == null ? "No AIC data for this day" : undefined}>{r.aic == null ? "–" : f.aic(r.aic)}</td>
              <td>{f.tokens(r.tok)}</td><td>{cached(r.cr, r.inp)}</td><td>{f.count(r.rq)}</td>
            </tr>
          ))}
          {!rows.length && <tr><td colSpan={5} className="empty">No usage in this range</td></tr>}
        </tbody>
      </table>
    </div>
  );
}

function Tile({ l, v, d }: { l: string; v: string; d?: string }) {
  return <div className="tile"><div className="label">{l}</div><div className="v num">{v}</div>{d && <div className="label">{d}</div>}</div>;
}
function Mini({ l, v, c }: { l: string; v: string; c?: string }) {
  return <div><div className="label">{l}</div><div className="num" style={{ fontWeight: 600, fontSize: 15, color: c }}>{v}</div></div>;
}
function Legend({ groups, color, dim = "model", children }: { groups: string[]; color: (g: string) => string; dim?: Dim; children?: React.ReactNode }) {
  return <div className="legend">{groups.map((g) => <span key={g}><span className="swatch" style={{ background: color(g) }} />{groupLabel(g, dim)}</span>)}{children}</div>;
}
function LineKey({ color, dash, label }: { color: string; dash?: string; label: string }) {
  return <span><svg width="16" height="8" aria-hidden style={{ marginRight: 5, verticalAlign: "middle" }}><line x1="1" x2="15" y1="4" y2="4" stroke={color} strokeWidth={2} strokeDasharray={dash} strokeLinecap="round" /></svg>{label}</span>;
}

interface PaceSeries {
  pace: Pace;
  days: number[];
  used: number[];
  prev: number[];
  hasPrev: boolean;
  scale: { max: number; step: number };
}

function paceSeries(snap: Snapshot): PaceSeries {
  const p = snap.pace;
  const days = cycleDays(p.cycle);
  const byDay = cycleDayTotals(snap.records, p.cycle);
  let cum = 0;
  const used = days.filter((d) => d <= snap.now).map((d) => (cum += byDay.get(dayKey(d)) ?? 0));
  const prevCycle = previousCycle(p.cycle);
  const prevBy = cycleDayTotals(snap.records, prevCycle);
  let pc = 0;
  const prev = cycleDays(prevCycle).slice(0, days.length).map((d) => (pc += prevBy.get(dayKey(d)) ?? 0));
  const hasPrev = pc > 0;
  const scale = f.niceScale(Math.max(p.allowance ?? 0, p.projected, p.used, hasPrev ? pc : 0, 1e-9) * 1.04, 4);
  return { pace: p, days, used, prev, hasPrev, scale };
}

const paceSummary = (p: Pace) => {
  const allowance = p.allowance != null ? ` of ${f.aic(p.allowance)} Allowance` : "";
  return `Pace chart: ${f.aic(p.used)} AIC used${allowance} this Cycle, projected ${f.aic(p.projected)} AIC.`;
};

function PaceChart({ snap }: { snap: Snapshot }) {
  const series = useMemo(() => paceSeries(snap), [snap]);
  const [box, W] = useWidth<HTMLDivElement>(900);
  const p = series.pace;
  const gap = p.unlogged;
  const over = p.allowance != null && p.projected > p.allowance;
  return (
    <div ref={box} className="measure" style={{ position: "relative" }}>
      <PaceSvg series={series} width={W} />
      <div className="legend">
        <LineKey color="var(--accent)" label="Logged AIC" />
        {gap > UNLOGGED_MIN_AIC && <span><span className="swatch" style={{ background: "var(--text)", borderRadius: 8 }} />GitHub total (+{f.aic(gap)} not in local logs)</span>}
        {p.allowance != null && <LineKey color="var(--muted)" dash="4 3" label="Even Pace" />}
        <LineKey color={over ? "var(--orange)" : "var(--accent)"} dash="5 3" label={`Projected → ${f.aic(p.projected)}`} />
        {series.hasPrev && <LineKey color="var(--faint)" dash="1 3" label={`Last Cycle (same day: ${f.aic(series.prev[Math.max(0, series.used.length - 1)] ?? 0)})`} />}
      </div>
    </div>
  );
}

function PaceSvg({ series, width: W }: { series: PaceSeries; width: number }) {
  const tip = useTip();
  const H = 210, pad = { l: 34, r: 8, t: 12, b: 20 };
  const { pace: p, days, used, prev, hasPrev, scale } = series;
  const gap = p.unlogged;
  const max = scale.max;
  const last = used.length - 1;
  const x = (i: number) => pad.l + (i / Math.max(1, days.length - 1)) * (W - pad.l - pad.r);
  const y = (v: number) => pad.t + (1 - v / max) * (H - pad.t - pad.b);
  const line = (vals: number[], start = 0) => vals.map((v, i) => `${i ? "L" : "M"}${x(i + start).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  return (
    <>
      <svg role="img" aria-label={paceSummary(p)} viewBox={`0 0 ${W} ${H}`} width={W} height={H} style={{ display: "block", overflow: "visible" }}
        onMouseMove={(e) => {
          const rect = (e.currentTarget as SVGSVGElement).getBoundingClientRect();
          const i = Math.round(((e.clientX - rect.left) / rect.width * W - pad.l) / (W - pad.l - pad.r) * (days.length - 1));
          if (i < 0 || i >= days.length) return tip.hide();
          tip.show(e, <><b>{f.weekdayDate(days[i])}</b>
            {i <= last && <div className="num">Logged: {f.aic(used[i])}</div>}
            {i === last && gap > UNLOGGED_MIN_AIC && <div className="num">GitHub total: {f.aic(p.used)}</div>}
            {p.allowance != null && <div className="num muted">Even pace: {f.aic((p.allowance * (i + 1)) / days.length)}</div>}
            {hasPrev && prev[i] != null && <div className="num muted">Last Cycle: {f.aic(prev[i])}</div>}</>);
        }} onMouseLeave={tip.hide}>
        {Array.from({ length: Math.round(max / scale.step) + 1 }, (_, i) => i * scale.step).map((v, i) => (
          <g key={v}>
            <line x1={pad.l} x2={W - pad.r} y1={y(v)} y2={y(v)} stroke="var(--line)" strokeDasharray={i ? "2 3" : undefined} />
            <text x={pad.l - 6} y={y(v) + 3.5} textAnchor="end" fontSize={10} fill="var(--faint)">{f.axisValue(v, scale.step, "aic")}</text>
          </g>
        ))}
        {p.allowance != null && (
          <>
            <line x1={pad.l} x2={W - pad.r} y1={y(p.allowance)} y2={y(p.allowance)} stroke="var(--red)" strokeOpacity={0.6} />
            <text x={pad.l + 4} y={y(p.allowance) - 5} fontSize={10} fill="var(--muted)">Allowance {f.aic(p.allowance)}</text>
            <path d={`M${x(0)},${y(p.allowance / days.length)} L${x(days.length - 1)},${y(p.allowance)}`} stroke="var(--muted)" strokeOpacity={0.8} strokeDasharray="4 3" fill="none" />
          </>
        )}
        {last >= 0 && <path d={`M${x(last)},${y(p.used)} L${x(days.length - 1)},${y(Math.max(p.used, p.projected))}`} stroke={p.allowance != null && p.projected > p.allowance ? "var(--orange)" : "var(--accent)"} strokeWidth={1.5} strokeDasharray="5 3" fill="none" />}
        {hasPrev && <path d={line(prev)} stroke="var(--faint)" strokeWidth={1.5} fill="none" strokeDasharray="1 3" strokeLinecap="round" />}
        <path d={line(used)} stroke="var(--accent)" strokeWidth={2.5} fill="none" strokeLinejoin="round" />
        {last >= 0 && <circle cx={x(last)} cy={y(used[last])} r={3.5} fill="var(--accent)" />}
        {last >= 0 && gap > UNLOGGED_MIN_AIC && (
          <g>
            <line x1={x(last)} x2={x(last)} y1={y(used[last])} y2={y(p.used)} stroke="var(--muted)" strokeDasharray="2 2" />
            <circle cx={x(last)} cy={y(p.used)} r={4} fill="var(--panel)" stroke="var(--text)" strokeWidth={1.5} />
            <text x={x(last) + 8} y={y(p.used) - 6} fontSize={10} fill="var(--muted)">GitHub {f.aic(p.used)}</text>
          </g>
        )}
        {days.map((d, i) => ((i % 7 === 0 && i < days.length - 4) || i === days.length - 1) && (
          <text key={d} x={x(i)} y={H - 4} textAnchor="middle" fontSize={9.5} fill="var(--faint)">{f.monthDay(d)}</text>
        ))}
      </svg>
      {tip.el}
    </>
  );
}


function ShareBar({ v, color }: { v: number | null; color: string }) {
  if (v == null) return <td className="faint">–</td>;
  return (
    <td>
      <span className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
        <span className="share-track"><span style={{ width: `${Math.min(100, v * 100)}%`, background: color }} /></span>
        <span className="num" style={{ minWidth: 34 }}>{f.pct(v)}</span>
      </span>
    </td>
  );
}

function Models({ snap, iv, clients }: P) {
  const color = useMemo(() => colorMap(snap), [snap]);
  const [sort, setSort] = useState<"aic" | "tok" | "rq" | "per" | "eff">("aic");
  const gap = unloggedIn(snap, iv, clients);
  const { rows, base } = useMemo(() => {
    const by = (m: Metric) => new Map(breakdown(snap.records, m, "model", iv, clients).map((r) => [r.key, r.value]));
    const aic = by("aic"), tok = by("totalTokens"), rq = by("requests");
    const base = [...aic.values()].reduce((a, b) => a + b, 0) + gap;
    const tokSum = [...tok.values()].reduce((a, b) => a + b, 0);
    const keys = [...new Set([...aic.keys(), ...tok.keys(), ...rq.keys()])];
    const rows = keys.map((k) => {
      const a = aic.get(k);
      return {
        k, hasAic: a != null, aic: a ?? 0, share: a != null && base ? a / base : null, tok: tok.get(k) ?? 0, tokShare: tokSum ? (tok.get(k) ?? 0) / tokSum : null,
        rq: rq.get(k) ?? 0, per: a != null && (rq.get(k) ?? 0) > 0 ? a / rq.get(k)! : null,
        eff: a != null && (tok.get(k) ?? 0) > 0 ? a / ((tok.get(k) ?? 0) / 1e6) : null,
      };
    }).sort((a, b) => ((b[sort] ?? -1) as number) - ((a[sort] ?? -1) as number));
    return { rows, base };
  }, [snap, iv, clients, sort, gap]);
  const th = (k: typeof sort, t: string, title?: string) => (
    <th aria-sort={sort === k ? "descending" : "none"} title={title}><button className="th-btn" onClick={() => setSort(k)}>{t}{sort === k ? " ↓" : ""}</button></th>
  );
  return (
    <div className="grid">
      <div className="card" style={{ padding: 0, overflow: "auto" }}>
        <table className="data">
          <thead><tr>
            <th>Model</th>{th("aic", "AIC")}<th>Share of AIC</th><th>Share of tokens</th>{th("tok", "Tokens")}{th("rq", "Requests")}
            {th("per", "AIC / request")}{th("eff", "AIC / 1M tokens")}
          </tr></thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.k}>
                <td><span className="swatch" style={{ background: color(r.k) }} />{modelName(r.k)}</td>
                <td title={r.hasAic ? undefined : "No AIC data"}>{r.hasAic ? f.aic(r.aic) : "–"}</td>
                <ShareBar v={r.share} color={color(r.k)} />
                <ShareBar v={r.tokShare} color={color(r.k)} />
                <td>{f.tokens(r.tok)}</td><td>{f.count(r.rq)}</td>
                <td>{r.per != null ? f.aic(r.per) : "–"}</td>
                <td>{r.eff != null ? f.aic(r.eff) : "–"}</td>
              </tr>
            ))}
            {gap > 0 && (
              <tr title={UNLOGGED_NOTE}>
                <td className="muted"><span className="swatch" style={{ background: "var(--faint)" }} />{UNLOGGED}</td>
                <td>{f.aic(gap)}</td><ShareBar v={base ? gap / base : null} color="var(--faint)" />
                <td colSpan={5} className="faint" style={{ textAlign: "left" }}>No per-model detail</td>
              </tr>
            )}
            {!rows.length && <tr><td colSpan={8} className="empty">No usage in this range</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}


function Projects({ snap, iv, clients }: P) {
  const color = useMemo(() => colorMap(snap), [snap]);
  const burst = snap.settings.notifications.burstAic;
  const [project, setProject] = useState<string | null>(null);
  const [sort, setSort] = useState<"start" | "aic" | "tokens">("start");
  const sessions = useMemo(() => sessionSummaries(snap.records, iv, clients), [snap, iv, clients]);
  const projects = useMemo(() => projectTotals(sessions), [sessions]);
  const gap = unloggedIn(snap, iv, clients);
  const share = new Map(projectShares(projects, gap).map((r) => [r.key, r.share]));
  const shown = sessions.filter((s) => !project || s.project === project)
    .sort((a, b) => (sort === "start" ? b.start - a.start : sort === "aic" ? b.aic - a.aic : b.tokens - a.tokens));
  const LIMIT = 200;
  const th = (k: typeof sort, t: string) => (
    <th aria-sort={sort === k ? "descending" : "none"}><button className="th-btn" onClick={() => setSort(k)}>{t}{sort === k ? " ↓" : ""}</button></th>
  );
  return (
    <div className="grid">
      <div className="card" style={{ padding: 0, overflow: "auto" }}>
        <table className="data">
          <thead><tr><th>Project</th><th>AIC</th><th>Share of AIC</th><th>Tokens</th><th>Sessions</th><th>AIC / session</th></tr></thead>
          <tbody>
            {projects.map(({ project: k, ...p }) => (
              <tr key={k} onClick={() => setProject(project === k ? null : k)} aria-selected={project === k} tabIndex={0}
                onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && setProject(project === k ? null : k)}
                style={{ cursor: "pointer", background: project === k ? "var(--accent-soft)" : undefined }}>
                <td>{k}</td><td>{f.aic(p.aic)}</td><ShareBar v={share.get(k) || null} color="var(--accent)" />
                <td>{f.tokens(p.tokens)}</td><td>{f.count(p.sessions)}</td><td>{f.aic(p.aic / p.sessions)}</td>
              </tr>
            ))}
            {gap > 0 && (
              <tr className="muted" title={UNLOGGED_NOTE}>
                <td>{UNLOGGED}</td><td>{f.aic(gap)}</td><ShareBar v={share.get(UNLOGGED) ?? null} color="var(--faint)" /><td colSpan={3} />
              </tr>
            )}
            {!projects.length && !gap && <tr><td colSpan={6} className="empty">No sessions in this range</td></tr>}
          </tbody>
        </table>
      </div>
      <div className="row wrap" style={{ gap: 8 }}>
        <b>Sessions{project ? ` in ${project}` : ""}</b>
        {project && <button className="link" onClick={() => setProject(null)}>Show all</button>}
        <span className="grow" />
        <span className="label">{shown.length > LIMIT ? `Newest ${LIMIT} of ${f.count(shown.length)}` : f.plural(shown.length, "session")}</span>
      </div>
      <div className="card" style={{ padding: 0, overflow: "auto" }}>
        <table className="data">
          <thead><tr>{th("start", "Started")}<th style={{ textAlign: "left" }}>Session</th><th style={{ textAlign: "left" }}>Project</th><th style={{ textAlign: "left" }}>Client</th><th style={{ textAlign: "left" }}>Models</th>{th("aic", "AIC")}{th("tokens", "Tokens")}</tr></thead>
          <tbody>
            {shown.slice(0, LIMIT).map((s) => {
              const runaway = burst > 0 && s.peak >= burst;
              const models = [...s.models].sort((a, b) => b[1] - a[1]).map(([k]) => k);
              return (
                <tr key={s.client + s.id} title={runaway ? `Runaway Session: ${f.aic(s.peak)} AIC within one hour` : undefined}>
                  <td>{f.weekdayDate(s.start)} {new Date(s.start).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}</td>
                  <td style={{ maxWidth: 240, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", textAlign: "left" }} title={s.label ? `${s.label} (${s.id})` : s.id}>{s.label ?? s.id.slice(0, 8)}</td>
                  <td style={{ maxWidth: 180, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", textAlign: "left" }} title={s.project}>{s.project}</td>
                  <td style={{ textAlign: "left" }}>{s.client}</td>
                  <td style={{ textAlign: "left", whiteSpace: "nowrap" }}>{models.slice(0, 2).map((k) => <span key={k} style={{ marginRight: 8 }}><span className="swatch" style={{ background: color(k) }} />{modelName(k)}</span>)}{models.length > 2 ? `+${models.length - 2}` : ""}</td>
                  <td>{runaway && <span style={{ color: "var(--orange-text)" }} aria-label="Runaway Session">▲ </span>}{s.hasAic ? f.aic(s.aic) : "–"}</td>
                  <td>{f.tokens(s.tokens)}</td>
                </tr>
              );
            })}
            {!shown.length && <tr><td colSpan={7} className="empty">No sessions in this range</td></tr>}
          </tbody>
        </table>
      </div>
      {burst > 0 && <p className="faint" style={{ fontSize: 12 }}>▲ Runaway Session: {f.aic(burst)}+ AIC within one hour</p>}
    </div>
  );
}


function exportCsv(snap: Snapshot, iv: Interval, clients: Set<Client>, range: RangeKey): Promise<string> {
  return api.exportCsv(`copilot-usage-${range}-${dayKey(snap.now)}.csv`, usageCsv(snap.records, iv, clients));
}
