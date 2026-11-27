import type React from "react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { addDays, anchorDayKey, breakdown, cycleDayTotals, cycleDays, DAY, dailyStacks, dayKey, type Metric, modelName, rangeDays, startOfDay, todayInterval, total, UNLOGGED, UNLOGGED_MIN_AIC, weekLayout, withUnlogged, yesterdayInterval } from "./analytics";
import { api } from "./backend";
import MenuBarSettings from "./MenuBarSettings";
import { Amount, colorMap, CopilotIcon, CycleSummary, CycleSummaryView, Icon, Seg, signInCopy, SignInStatus, StackedBars, StatusChip, TodayMeter, UNLOGGED_NOTE } from "./components";
import * as f from "./format";
import { parseDecimal } from "./settingsModel";
import { opensExpanded, useSnapshot } from "./state";
import type { Pace, Snapshot } from "./types";

const WIDTH = 360;
const CYCLE_MONTH_OFFSET = 3 * DAY;
const SPARSE_MIN_DAYS = 3;
const SPARSE_USED_SHARE = 0.05;
const TRACE_AIC = 0.05;
const EVEN_BAND_LOW = 0.5;
const EVEN_BAND_HIGH = 1.5;
const NO_ALLOWANCE_STEPS = 4;
type View = "overview" | "calendar" | "settings";
export type PopoverStart = "overview" | "expanded" | "calendar";

const REMEMBER = "copilotbar.popover.view";
function recall(remember: boolean): { calendar: boolean; expanded: boolean | null } {
  try { const v = remember && localStorage.getItem(REMEMBER); if (v) return JSON.parse(v); } catch {}
  return { calendar: false, expanded: null };
}

export default function Popover({ start, remember = true }: { start?: PopoverStart; remember?: boolean }) {
  const { snap, busy, refreshedAt, refresh, update, refreshFailed, loadFailed, retry } = useSnapshot();
  const [view, setView] = useState<View>(() => (recall(remember).calendar ? "calendar" : "overview"));
  const [expanded, setExpanded] = useState<boolean | null>(() => recall(remember).expanded);
  const [metric, setMetric] = useState<Metric>("aic");
  const ref = useRef<HTMLDivElement>(null);
  const calBtn = useRef<HTMLButtonElement>(null);
  const isExpanded = expanded ?? (snap ? opensExpanded(snap.settings) : false);

  useEffect(() => {
    if (!remember || !snap || view === "settings") return;
    try { localStorage.setItem(REMEMBER, JSON.stringify({ calendar: view === "calendar", expanded: isExpanded })); } catch {}
  }, [view, isExpanded, snap != null]);
  useEffect(() => api.on("popover-shown", () => { const r = recall(remember); setView(r.calendar ? "calendar" : "overview"); if (r.expanded != null) setExpanded(r.expanded); }), []);
  useEffect(() => {
    if (!start || !snap) return;
    setView(start === "calendar" ? "calendar" : "overview");
    setExpanded(start === "expanded");
  }, [start, snap != null]);

  const toCalendar = () => setView((v) => (v === "calendar" ? "overview" : "calendar"));
  const back = () => { setView("overview"); requestAnimationFrame(() => calBtn.current?.focus()); };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (e.key === "Escape") { if (view === "overview") api.hidePopover(); else back(); }
      if (mod && e.key === "r") { e.preventDefault(); void refresh(); }
      if (mod && e.key === ",") { e.preventDefault(); setView("settings"); }
      if (mod && e.key === "d") { e.preventDefault(); api.openDashboard(); }
      if (mod && e.key === "k") { e.preventDefault(); view === "calendar" ? back() : setView("calendar"); }
      if (mod && e.key === "i" && view === "overview") { e.preventDefault(); setExpanded(!isExpanded); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, refresh, isExpanded]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    let sent = { w: -1, h: -1 };
    let seq = 0;
    const ro = new ResizeObserver(() => {
      const w = el.offsetWidth;
      const h = Math.ceil(el.getBoundingClientRect().height);
      if (w === sent.w && h === sent.h) return;
      sent = { w, h };
      const id = ++seq;
      void api.resizePopover(w, h).then((applied) => {
        if (id === seq) document.documentElement.classList.toggle("scrolls", applied < h - 0.5);
      });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [snap != null]);

  if (!snap) return <div ref={ref} className="popover-root"><div className="popover-panel" style={{ width: WIDTH }}>{loadFailed ? <LoadFailed onRetry={retry} /> : <Skeleton />}</div></div>;
  return (
    <div ref={ref} className="popover-root">
      <div className="popover-panel" style={{ width: WIDTH }}>
        <div className="popover-body">
          {view === "settings" ? (
            <div className="view-in" key="settings">
              <QuickSettings snap={snap} onBack={() => setView("overview")} update={update} />
            </div>
          ) : (
            <>
              <Header snap={snap} busy={busy} calendar={view === "calendar"} insights={view === "overview" && isExpanded} onRefresh={refresh}
                onCalendar={view === "calendar" ? back : toCalendar} calRef={calBtn}
                onInsights={() => { if (view === "calendar") { setView("overview"); setExpanded(true); } else setExpanded(!isExpanded); }} />
              {refreshFailed && <p className="help" role="alert" style={{ margin: "0 0 8px", userSelect: "text" }}>Couldn't refresh. Try again.</p>}
              {!snap.settings.onboarded && <Welcome snap={snap} onDone={(b) => update(b > 0 ? { onboarded: true, allowanceOverride: b } : { onboarded: true })} onSettings={() => setView("settings")} />}
              <div className="view-in" key={view}>
                {view === "calendar" ? (
                  <CalendarView snap={snap} />
                ) : (
                  <>
                    <Overview snap={snap} />
                    {isExpanded && <div className="fade-in insights"><Details snap={snap} metric={metric} setMetric={setMetric} /></div>}
                  </>
                )}
              </div>
              <Problems snap={snap} />
              <Footer snap={snap} busy={busy} justRefreshed={refreshedAt != null} onSettings={() => setView("settings")} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function LoadFailed({ onRetry }: { onRetry: () => void }) {
  return (
    <div role="alert" style={{ display: "grid", gap: 8, justifyItems: "start", padding: "12px 0" }}>
      <span>Couldn't read usage.</span>
      <button className="btn" onClick={onRetry}>Retry</button>
    </div>
  );
}

const calendarMonthTitle = (cycle: { start: number }) => f.monthName(cycle.start + CYCLE_MONTH_OFFSET);

function Skeleton() {
  return (
    <div role="status" aria-label="Loading usage">
      <div className="row" style={{ marginBottom: 10 }}>
        <span className="skel" style={{ width: 17, height: 17, borderRadius: 5 }} />
        <span className="skel" style={{ width: 80, height: 14 }} />
      </div>
      <div className="skel" style={{ height: 64, borderRadius: 12, marginBottom: 10 }} />
      <div className="skel" style={{ height: 96, borderRadius: 12 }} />
    </div>
  );
}

function Header({ snap, busy, calendar, insights, onRefresh, onCalendar, onInsights, calRef }: {
  snap: Snapshot; busy: boolean; calendar: boolean; insights: boolean; onRefresh: () => void; onCalendar: () => void; onInsights: () => void;
  calRef: React.RefObject<HTMLButtonElement | null>;
}) {
  return (
    <div className="row" style={{ marginBottom: "var(--space-2)", gap: 6 }}>
      <CopilotIcon size={17} />
      <b style={{ fontSize: 14, flex: "none" }}>{calendar ? calendarMonthTitle(snap.pace.cycle) : "Copilot"}</b>
      {!calendar && snap.pace.status !== "noAllowance" && <span style={{ flex: "none" }}><StatusChip snap={snap} /></span>}
      <span className="grow" />
      <span className="view-toggles" role="group" aria-label="Views">
        <button className={`iconbtn${insights ? " on" : ""}`} aria-pressed={insights} onClick={onInsights}
          title={insights ? "Hide insights (⌘I)" : "Insights: daily chart and models (⌘I)"} aria-label="Insights"><Icon.insights /></button>
        <button ref={calRef} className={`iconbtn${calendar ? " on" : ""}`} aria-pressed={calendar} onClick={onCalendar}
          title={calendar ? "Close calendar (Esc)" : "Calendar (⌘K)"} aria-label="Calendar"><Icon.calendar /></button>
        <button className={busy ? "iconbtn on" : "iconbtn"} onClick={onRefresh} disabled={busy} aria-busy={busy} title="Refresh (⌘R)" aria-label="Refresh"><Icon.refresh spinning={busy} /></button>
      </span>
    </div>
  );
}

function Footer({ snap, busy, justRefreshed, onSettings }: { snap: Snapshot; busy: boolean; justRefreshed: boolean; onSettings: () => void }) {
  return (
    <div className="row" style={{ borderTop: "1px solid var(--line)", marginTop: 12, paddingTop: 8 }}>
      <FooterStatus snap={snap} busy={busy} justRefreshed={justRefreshed} />
      <button className="iconbtn" style={{ flex: "none" }} onClick={() => api.openDashboard()} title="Open dashboard (⌘D)" aria-label="Open dashboard"><Icon.chart /></button>
      <button className="iconbtn" style={{ flex: "none" }} onClick={onSettings} title="Settings (⌘,)" aria-label="Settings"><Icon.gear /></button>
    </div>
  );
}

function FooterStatus({ snap, busy, justRefreshed }: { snap: Snapshot; busy: boolean; justRefreshed: boolean }) {
  const st = snap.status, err = st.planError;
  const style: React.CSSProperties = { fontSize: 11, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" };
  if (busy && !st.connectingSince) return <span className="faint grow" style={style} role="status">Refreshing…</span>;
  if (justRefreshed && !err) {
    return <span className="grow footer-ok" style={style} role="status">✓ {snap.githubPlan ? `@${snap.githubPlan.login} · ` : "Local logs · "}updated just now</span>;
  }
  if (st.connectingSince) return <span className="faint grow" style={style}>Connecting to GitHub…</span>;
  if (err?.offline) {
    return (
      <span className="grow" style={{ ...style, color: "var(--orange-text)" }} title={`${err.message}. Local logs still work. Press ⌘R to retry.`}>
        Offline · {st.lastRefresh ? `updated ${f.ago(st.lastRefresh, snap.now)}` : "local logs only"}
      </span>
    );
  }
  const c = signInCopy(st.signIn);
  if (err || c.problem === "fix") {
    return (
      <span className="grow row" style={{ ...style, gap: 6 }} title={err?.message ?? c.title}>
        <span style={{ color: "var(--orange-text)", overflow: "hidden", textOverflow: "ellipsis" }}>{err ? `${c.short} · local logs only` : c.short}</span>
        {c.problem === "fix" && <button className="link" style={{ fontSize: 11, flex: "none" }} onClick={() => api.openDashboard("settings")}>How to fix…</button>}
      </span>
    );
  }
  return <span className="faint grow" style={style}>{snap.githubPlan ? `@${snap.githubPlan.login} · ` : "Local logs · "}updated {f.ago(st.lastRefresh, snap.now)}</span>;
}

function Problems({ snap }: { snap: Snapshot }) {
  const err = snap.status.planError;
  const since = snap.status.connectingSince;
  const [, tick] = useState(0);
  useEffect(() => { if (since) { const t = setInterval(() => tick((n) => n + 1), 1000); return () => clearInterval(t); } }, [since]);
  if (since) {
    if (Date.now() - since <= 4000) return null;
    return (
      <div className="callout" style={{ background: "var(--card)", marginTop: 10, fontSize: 12 }}>
        <span>⏳</span>
        <span>Waiting for the GitHub CLI (<code>gh</code>) to answer.</span>
      </div>
    );
  }
  if (err || snap.records.length) return null;
  return <div className="faint" style={{ fontSize: 11.5, textAlign: "center", marginTop: 10 }}>No Copilot usage yet.</div>;
}

function Welcome({ snap, onDone, onSettings }: { snap: Snapshot; onDone: (allowance: number) => void; onSettings: () => void }) {
  const found = snap.status.cliFiles;
  const [allowance, setAllowance] = useState("");
  const allowanceValue = parseDecimal(allowance);
  return (
    <div className="card" style={{ margin: "4px 0 12px", background: "var(--accent-soft)" }}>
      <b>Welcome to CopilotBar 👋</b>
      <ul style={{ margin: "6px 0 10px", paddingLeft: 18, display: "grid", gap: 4, fontSize: 12.5 }}>
        <li>{found ? <>Found <b>{found}</b> Copilot sessions on this Mac.</> : <>No Copilot logs yet.</>}</li>
        <li><SignInStatus signIn={snap.status.signIn} /></li>
        <li>Pick a popover style and menu bar look in <button className="link" onClick={onSettings}>Settings</button>.</li>
      </ul>
      <label className="row" style={{ fontSize: 12.5, marginBottom: 10 }}>
        <span className="grow">Allowance{snap.githubPlan?.entitlement ? ` (your Entitlement is ${f.aic(snap.githubPlan.entitlement)})` : ""}</span>
        <input className="field num" style={{ width: 80, textAlign: "right" }} inputMode="decimal" placeholder={snap.githubPlan?.entitlement ? String(snap.githubPlan.entitlement) : "e.g. 300"}
          value={allowance} aria-invalid={allowance !== "" && allowanceValue === null} onChange={(e) => setAllowance(e.target.value.replace(/[^0-9.]/g, ""))} aria-label="Allowance in AIC" />
        <span className="faint">AIC</span>
      </label>
      <div className="row"><span className="grow" /><button className="btn primary" disabled={allowance !== "" && allowanceValue === null} onClick={() => onDone(allowanceValue ?? 0)}>Got it</button></div>
    </div>
  );
}

const resetLabel = (snap: Snapshot) => f.resetDay(snap.pace.cycle.end);


const STATS_ROW = {
  display: "flex", justifyContent: "space-between", textAlign: "center",
  borderTop: "1px solid color-mix(in srgb, var(--line) 55%, transparent)", marginTop: "var(--space-3)", paddingTop: "var(--space-3)",
} as const;

function isSparse(p: Pace): boolean {
  const barelyUsed = p.used < SPARSE_USED_SHARE * (p.allowance ?? Infinity);
  return p.elapsedDays < SPARSE_MIN_DAYS || (barelyUsed && p.activeDays < SPARSE_MIN_DAYS);
}

function Overview({ snap }: { snap: Snapshot }) {
  const p = snap.pace;
  const { today, yday, cycleTokens } = useMemo(() => ({
    today: total(snap.records, "aic", todayInterval(snap.now)),
    yday: total(snap.records, "aic", yesterdayInterval(snap.now)),
    cycleTokens: total(snap.records, "totalTokens", { start: p.cycle.start, end: Math.min(snap.now, p.cycle.end) }),
  }), [snap, p.cycle.start, p.cycle.end]);
  const sparse = isSparse(p);
  const dailyTokens = f.tokens(cycleTokens / Math.max(1, p.elapsedDays));
  return (
    <>
      <CycleSummaryView snap={snap} />
      <div className="card" style={{ marginTop: "var(--space-3)" }}>
        <div className="row" style={{ marginBottom: "var(--space-1)" }}><span className="caps">Today</span><span className="grow" /><span className="label">yesterday {f.aic(yday)}</span></div>
        <TodayMeter pace={p} today={Math.max(today, p.usedToday)} />
        <div style={STATS_ROW}>
          {sparse ? (
            <>
              <Stat v={p.allowance != null ? f.aic(Math.max(0, p.allowance - p.used)) : "–"} l={`left until ${resetLabel(snap)}`} />
              <Stat v={f.aic(p.avgDaily)} l="daily avg" />
              <Stat v={dailyTokens} l="daily tokens" />
            </>
          ) : (
            <>
              <Stat v={f.aic(p.avgDaily)} l="daily avg" />
              <Stat v={f.aic(p.avgRecent)} l="weekly avg" />
              <Stat v={dailyTokens} l="daily tokens" />
            </>
          )}
        </div>
      </div>
    </>
  );
}

function Stat({ v, l, color }: { v: string; l: string; color?: string }) {
  return (
    <div>
      <div className="num" style={{ fontSize: 16, fontWeight: 600, color }}>{v}</div>
      <div className="label">{l}</div>
    </div>
  );
}


function Details({ snap, metric, setMetric }: { snap: Snapshot; metric: Metric; setMetric: (m: Metric) => void }) {
  const p = snap.pace;
  const color = useMemo(() => colorMap(snap), [snap]);
  const model = useMemo(() => {
    const shown = { start: p.cycle.start, end: addDays(startOfDay(snap.now), 1) };
    const stacks = dailyStacks(snap.records, metric, shown, "model", 6, undefined, rangeDays(shown, p.cycle));
    const rows = breakdown(snap.records, metric, "model", { start: p.cycle.start, end: p.cycle.end });
    const withGap = metric === "aic" ? withUnlogged(rows, p.unlogged) : rows;
    const unlogged = withGap.find((m) => m.key === UNLOGGED);
    const all = withGap.filter((m) => m !== unlogged);
    const rest = all.slice(5).reduce((s, m) => ({ value: s.value + m.value, share: s.share + m.share }), { value: 0, share: 0 });
    return { stacks, unlogged, models: all.slice(0, 5), rest, otherCount: all.length - 5 };
  }, [snap, metric, p.cycle, p.unlogged]);
  const aic = metric === "aic";
  const { stacks, unlogged, models, rest, otherCount } = model;
  const fmt = (v: number) => (aic ? f.aic(v) : f.tokens(v));
  return (
    <div style={{ display: "grid", gap: "var(--space-3)" }}>
      <div className="card">
        <div className="row" style={{ marginBottom: 6 }}>
          <b>This Cycle</b><span className="grow" />
          <Seg label="Measure" value={metric} options={[["aic", "AIC"], ["totalTokens", "Tokens"]]} onChange={setMetric} />
        </div>
        <StackedBars days={stacks.days} groups={stacks.groups} metric={metric} color={color} height={110} width={308} dim="model"
          evenPerDay={aic && p.allowance ? p.allowance / p.totalDays : null} todayKey={dayKey(snap.now)} futureUntil={addDays(cycleDays(p.cycle).slice(-1)[0], 1)} />
        <div style={{ display: "grid", gap: 6, marginTop: 10 }}>
          {models.length === 0 && !unlogged && <span className="faint">{aic ? "No AIC used yet this Cycle." : "No usage yet this Cycle."}</span>}
          {models.map((m) => (
            <ShareRow key={m.key} label={modelName(m.key)} value={fmt(m.value)} share={m.share} color={color(m.key)} />
          ))}
          {rest.value > 0 && <ShareRow label={`Other (${otherCount})`} value={fmt(rest.value)} share={rest.share} color="var(--m7)" muted />}
          {unlogged && (
            <ShareRow label={UNLOGGED} value={f.aic(unlogged.value)} share={unlogged.share} color="var(--track)" muted
              title={UNLOGGED_NOTE} />
          )}
        </div>
      </div>
    </div>
  );
}

function ShareRow({ label, value, share, color, muted, title }: { label: string; value: string; share: number; color: string; muted?: boolean; title?: string }) {
  return (
    <div title={title}>
      <div className="row" style={{ fontSize: 11.5 }}>
        <span className={muted ? "muted" : undefined}>{label}</span><span className="grow" />
        <span className="num">{value}</span>
        <span className="num faint" style={{ width: 34, textAlign: "right" }}>{f.pct(share)}</span>
      </div>
      <div style={{ height: 4, borderRadius: 4, background: color, width: `${Math.min(100, share * 100).toFixed(1)}%`, marginTop: 2 }} />
    </div>
  );
}


function CyclePace({ snap }: { snap: Snapshot }) {
  const p = snap.pace;
  const reset = resetLabel(snap);
  if (p.allowance == null) {
    return (
      <div className="row cal-pace" style={{ alignItems: "baseline", gap: 4 }}>
        <Amount big={f.aic(p.used)} size={28} /><span className="muted">AIC this Cycle</span>
        <span className="grow" /><span className="faint" style={{ fontSize: 11.5 }}>resets {reset}</span>
      </div>
    );
  }
  return <div className="cal-pace"><CycleSummary pace={p} mode={snap.settings.cycleHeadline} capped={snap.allowanceCapped} /></div>;
}

function paceBand(aic: number, even: number | null, max: number): number {
  if (aic < TRACE_AIC) return 0;
  if (even == null) return 5 + Math.min(NO_ALLOWANCE_STEPS - 1, Math.floor((aic / max) * NO_ALLOWANCE_STEPS));
  if (aic < even * EVEN_BAND_LOW) return 1;
  if (aic <= even) return 2;
  return aic <= even * EVEN_BAND_HIGH ? 3 : 4;
}

interface CalendarDay { key: string; ts: number; aic: number }

function CalendarView({ snap }: { snap: Snapshot }) {
  const p = snap.pace;
  const color = useMemo(() => colorMap(snap), [snap]);
  const todayKey = dayKey(snap.now);
  const days = useMemo<CalendarDay[]>(() => {
    const totals = cycleDayTotals(snap.records, p.cycle);
    return cycleDays(p.cycle).map((t) => ({ key: dayKey(t), ts: t, aic: totals.get(dayKey(t)) ?? 0 }));
  }, [snap]);
  const anchor = anchorDayKey(days, todayKey, snap.now);
  const [sel, setSel] = useState(anchor);
  useEffect(() => setSel(anchor), [anchor]);
  const cells = useRef(new Map<string, HTMLButtonElement>());
  useEffect(() => { if (anchor) cells.current.get(anchor)?.focus({ preventScroll: true }); }, []);
  const even = p.allowance ? p.allowance / p.totalDays : null;
  const max = Math.max(...days.map((d) => d.aic), 1e-9);
  const { lead } = weekLayout(days.map((d) => d.ts), snap.settings.weekStartsMonday);
  const names = snap.settings.weekStartsMonday ? ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] : ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const elapsed = days.filter((d) => d.ts <= snap.now);
  const move = (by: number) => {
    const i = elapsed.findIndex((d) => d.key === sel);
    const next = elapsed[Math.min(elapsed.length - 1, Math.max(0, i + by))];
    if (next) { setSel(next.key); cells.current.get(next.key)?.focus(); }
  };
  const onKey = (e: React.KeyboardEvent) => {
    const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[e.key];
    if (step) { e.preventDefault(); move(step); }
  };
  const selDay = days.find((d) => d.key === sel) ?? elapsed[elapsed.length - 1];
  return (
    <>
      <CyclePace snap={snap} />
      <div style={{ display: "grid", gridTemplateColumns: "repeat(7,minmax(0,1fr))", gap: 4, fontSize: 10, textAlign: "center" }} className="faint">
        {names.map((n) => <div key={n}>{n}</div>)}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(7,minmax(0,1fr))", gap: 4, marginTop: 4 }} role="grid" aria-label="AIC per day" onKeyDown={onKey}>
        {Array.from({ length: lead }, (_, i) => <div key={"pad" + i} />)}
        {days.map((d, i) => {
          const future = d.ts > snap.now;
          const row = Math.floor((lead + i) / 7);
          const quiet = future && !days.some((o, j) => Math.floor((lead + j) / 7) === row && o.ts <= snap.now);
          const band = future ? 0 : paceBand(d.aic, even, max);
          const isSel = d.key === sel;
          return (
            <button key={d.key} ref={(el) => { if (el) cells.current.set(d.key, el); }}
              className={`cal-day band-${band}${future ? " future" : ""}${quiet ? " quiet" : ""}${d.key === todayKey ? " today" : ""}`} disabled={future}
              tabIndex={isSel ? 0 : -1} onClick={() => setSel(d.key)} title={`${f.weekdayDate(d.ts)} · ${f.aic(d.aic)} AIC`}
              aria-label={`${f.weekdayDate(d.ts)}, ${f.aic(d.aic)} AIC${even != null && d.aic > even ? ", above even pace" : ""}`} aria-pressed={isSel}>
              <span className="cal-date">{new Date(d.ts).getDate()}</span>
              {!future && <span className="cal-aic num">{d.aic < TRACE_AIC ? "" : f.cellAic(d.aic)}</span>}
            </button>
          );
        })}
      </div>
      <div className="faint row" style={{ fontSize: 10.5, marginTop: 8, gap: 6, flexWrap: "wrap" }}>
        {even != null ? (
          <>
            <span>Even Pace {f.short(even)}/day</span><span className="grow" />
            <span className="cal-sw band-1" />&lt;½×<span className="cal-sw band-2" />½–1×<span className="cal-sw band-3" />1–1.5×<span className="cal-sw band-4" />&gt;1.5×
          </>
        ) : <span>Deeper blue = more AIC</span>}
      </div>
      {p.unlogged > UNLOGGED_MIN_AIC && <div className="faint" style={{ fontSize: 10.5, marginTop: 3 }}>+{f.aic(p.unlogged)} AIC · {UNLOGGED}</div>}
      {selDay && <DayDetail snap={snap} day={selDay} even={even} isToday={selDay.key === todayKey} color={color} />}
    </>
  );
}

function DayDetail({ snap, day, even, isToday, color }: { snap: Snapshot; day: { ts: number; aic: number }; even: number | null; isToday: boolean; color: (k: string) => string }) {
  const iv = { start: day.ts, end: addDays(day.ts, 1) };
  const aicRows = breakdown(snap.records, "aic", "model", iv);
  const tokenRows = breakdown(snap.records, "totalTokens", "model", iv);
  const shown = aicRows.length ? aicRows : tokenRows;
  const inAic = aicRows.length > 0;
  const top = Math.max(...shown.map((m) => m.value), 1e-9);
  return (
    <div className="card fade-in" key={day.ts} style={{ marginTop: "var(--space-3)" }}>
      <div className="row" style={{ marginBottom: "var(--space-1)" }}><span className="caps">{isToday ? "Today" : f.longDate(day.ts)}</span></div>
      {inAic || day.aic > 0 ? (
        <div className="row" style={{ alignItems: "baseline", gap: 5 }}>
          <span className="num" style={{ fontSize: 19, fontWeight: 700 }}>{f.aic(day.aic)}</span>
          <span className="num faint" style={{ fontSize: 12, fontWeight: 500 }}>{even != null ? `/ ${f.short(even)} AIC even` : "AIC"}</span>
          <span className="grow" />
          {even != null && day.aic > 0 && (
            <span className="num" style={{ fontSize: 11.5, whiteSpace: "nowrap", color: day.aic > even ? "var(--orange-text)" : "var(--muted)" }}>
              {f.short(Math.abs(day.aic - even))} AIC {day.aic > even ? "over" : "under"}
            </span>
          )}
        </div>
      ) : shown.length > 0 ? (
        <div className="row" style={{ alignItems: "baseline", gap: 5 }}>
          <span className="num" style={{ fontSize: 19, fontWeight: 700 }}>{f.tokens(total(snap.records, "totalTokens", iv))}</span>
          <span className="num faint" style={{ fontSize: 12 }}>tokens · no AIC logged</span>
        </div>
      ) : null}
      <div className="faint num" style={{ fontSize: 11, margin: "6px 0" }}>
        {f.tokens(total(snap.records, "totalTokens", iv))} tokens · {f.tokens(total(snap.records, "cacheReadTokens", iv))} cached · {f.plural(total(snap.records, "requests", iv), "request")}
      </div>
      {shown.slice(0, 6).map((m) => (
        <div key={m.key} className="row" style={{ fontSize: 11.5, margin: "3px 0" }}>
          <span style={{ width: 118, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{modelName(m.key)}</span>
          <div className="grow" style={{ height: 6, borderRadius: 6, background: "var(--track)" }}>
            <div style={{ height: "100%", borderRadius: 6, background: color(m.key), width: `${(m.value / top) * 100}%` }} />
          </div>
          <span className="num" style={{ width: 52, textAlign: "right" }}>{inAic ? f.aic(m.value) : `${f.tokens(m.value)} tok`}</span>
        </div>
      ))}
      {!shown.length && <span className="faint">No recorded usage.</span>}
    </div>
  );
}


function QuickSettings({ snap, onBack, update }: { snap: Snapshot; onBack: () => void; update: (p: Partial<Snapshot["settings"]>) => void }) {
  return (
    <div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr auto 1fr", alignItems: "center", marginBottom: 12 }}>
        <button className="link" style={{ justifySelf: "start" }} onClick={onBack} autoFocus>‹ Back</button>
        <b>Settings</b>
        <button className="link row" style={{ justifySelf: "end", gap: 3 }} onClick={() => api.openDashboard("settings")}
          title="All settings: Allowance, notifications, logs, account" aria-label="More settings">
          More
          <svg width="9" height="9" viewBox="0 0 10 10" aria-hidden><path d="M3 1.5h5.5V7M8.5 1.5 1.5 8.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" /></svg>
        </button>
      </div>
      <div className="caps" style={{ margin: "4px 0 8px" }}>Menu bar</div>
      <MenuBarSettings snap={snap} update={update} compact />
    </div>
  );
}

