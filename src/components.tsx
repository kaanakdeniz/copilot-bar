import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { addDays, groupLabel, type DayStack, type Dim, type Metric } from "./analytics";
import * as f from "./format";
import type { MenuBarIcon, MenuBarLook, MenuBarNumber, Pace, PaceStatus, Settings, SignIn, Snapshot, UpdateState } from "./types";


const Cmd = ({ children }: { children: string }) => <code style={{ userSelect: "text" }}>{children}</code>;

const LOCAL_STILL_WORKS = "Local usage still works.";

export function signInCopy(signIn: SignIn): { title: string; short: string; next: ReactNode; problem: false | "fix" | "retry" } {
  switch (signIn.state) {
    case "signedIn":
      return { title: `Signed in as @${signIn.login}`, short: `@${signIn.login}`, next: null, problem: false };
    case "ghMissing":
      return { title: "The GitHub CLI isn't installed", short: "GitHub CLI not installed", next: <>Install it with <Cmd>brew install gh</Cmd>, then run <Cmd>gh auth login</Cmd>. {LOCAL_STILL_WORKS}</>, problem: "fix" };
    case "signedOut":
      return { title: "The GitHub CLI isn't signed in", short: "GitHub CLI not signed in", next: <>Run <Cmd>gh auth login</Cmd> in Terminal. {LOCAL_STILL_WORKS}</>, problem: "fix" };
    case "ssoRequired":
      return {
        title: "Your organisation's SSO hasn't authorised the GitHub CLI",
        short: "GitHub CLI needs SSO",
        next: <>Run <Cmd>gh auth refresh</Cmd>, and in the browser click <b>Authorize</b> next to your organisation. {LOCAL_STILL_WORKS}</>,
        problem: "fix",
      };
    case "accountMismatch":
      return {
        title: `GitHub CLI is signed in as @${signIn.ghLogin}, Copilot CLI as @${signIn.copilotLogin}`,
        short: `Plan data is @${signIn.ghLogin}'s, not @${signIn.copilotLogin}'s`,
        next: <>The plan shown is @{signIn.ghLogin}'s. Run <Cmd>{`gh auth switch --user ${signIn.copilotLogin}`}</Cmd> to see yours.</>,
        problem: "fix",
      };
    case "error":
      return { title: signIn.message, short: "GitHub error", next: <>Run <Cmd>gh auth status</Cmd> to check the GitHub CLI. {LOCAL_STILL_WORKS}</>, problem: "retry" };
    default:
      return { title: "Checking the GitHub CLI…", short: "Checking the GitHub CLI…", next: null, problem: false };
  }
}

export function SignInStatus({ signIn }: { signIn: SignIn }) {
  const c = signInCopy(signIn);
  return (
    <div role="status" style={{ display: "grid", gap: 2 }}>
      <b style={c.problem ? { color: "var(--orange-text)" } : undefined}>{c.title}</b>
      {c.next && <span className="help">{c.next}</span>}
    </div>
  );
}


function updateResult(update: UpdateState, onOpen: () => void): { text: string; action?: ReactNode; problem?: boolean } {
  switch (update.state) {
    case "checking":
      return { text: "Checking for updates…" };
    case "upToDate":
      return { text: "You're up to date." };
    case "available":
      return { text: `CopilotBar ${update.version} is available.`, action: <button className="btn primary" onClick={onOpen}>{`Update to ${update.version}`}</button> };
    case "downloading":
      return { text: `Downloading CopilotBar ${update.version}…` };
    case "installing":
      return { text: `Installing CopilotBar ${update.version}…` };
    case "error":
      return { text: update.message, problem: true, action: update.url ? <button className="btn" onClick={onOpen}>Open the release page</button> : undefined };
    default:
      return { text: "Not checked yet." };
  }
}

export const isUpdateBusy = (u: UpdateState) => u.state === "checking" || u.state === "downloading" || u.state === "installing";

export function UpdatesSection({ version, update, autoCheck, onCheck, onAutoCheck, onOpen }: {
  version: string; update: UpdateState; autoCheck: boolean; onCheck: () => void; onAutoCheck: (v: boolean) => void; onOpen: () => void;
}) {
  const r = updateResult(update, onOpen);
  return (
    <div className="section">
      <div className="setting">
        <div className="text">
          <div>CopilotBar {version}</div>
          <div className="help" role="status" style={r.problem ? { color: "var(--orange-text)" } : undefined}>{r.text}</div>
        </div>
        <span className="row" style={{ gap: 6 }}>
          {r.action}
          <button className="btn" disabled={isUpdateBusy(update)} onClick={onCheck}>Check for Updates</button>
        </span>
      </div>
      <div className="setting">
        <div className="text"><div>Automatically check for updates</div><div className="help">At launch and once a day.</div></div>
        <Toggle label="Automatically check for updates" checked={autoCheck} onChange={onAutoCheck} />
      </div>
    </div>
  );
}


/** Official Copilot glyph (Primer Octicons copilot-16, MIT). */
export function CopilotIcon({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor" aria-hidden style={{ flex: "none" }}>
      <CopilotGlyphPaths />
    </svg>
  );
}

function CopilotGlyphPaths() {
  return (
    <g fill="currentColor">
      <path d="M7.998 15.035c-4.562 0-7.873-2.914-7.998-3.749V9.338c.085-.628.677-1.686 1.588-2.065.013-.07.024-.143.036-.218.029-.183.06-.384.126-.612-.201-.508-.254-1.084-.254-1.656 0-.87.128-1.769.693-2.484.579-.733 1.494-1.124 2.724-1.261 1.206-.134 2.262.034 2.944.765.05.053.096.108.139.165.044-.057.094-.112.143-.165.682-.731 1.738-.899 2.944-.765 1.23.137 2.145.528 2.724 1.261.566.715.693 1.614.693 2.484 0 .572-.053 1.148-.254 1.656.066.228.098.429.126.612.012.076.024.148.037.218.924.385 1.522 1.471 1.591 2.095v1.872c0 .766-3.351 3.795-8.002 3.795Zm0-1.485c2.28 0 4.584-1.11 5.002-1.433V7.862l-.023-.116c-.49.21-1.075.291-1.727.291-1.146 0-2.059-.327-2.71-.991A3.222 3.222 0 0 1 8 6.303a3.24 3.24 0 0 1-.544.743c-.65.664-1.563.991-2.71.991-.652 0-1.236-.081-1.727-.291l-.023.116v4.255c.419.323 2.722 1.433 5.002 1.433ZM6.762 2.83c-.193-.206-.637-.413-1.682-.297-1.019.113-1.479.404-1.713.7-.247.312-.369.789-.369 1.554 0 .793.129 1.171.308 1.371.162.181.519.379 1.442.379.853 0 1.339-.235 1.638-.54.315-.322.527-.827.617-1.553.117-.935-.037-1.395-.241-1.614Zm4.155-.297c-1.044-.116-1.488.091-1.681.297-.204.219-.359.679-.242 1.614.091.726.303 1.231.618 1.553.299.305.784.54 1.638.54.922 0 1.28-.198 1.442-.379.179-.2.308-.578.308-1.371 0-.765-.123-1.242-.37-1.554-.233-.296-.693-.587-1.713-.7Z" />
      <path d="M6.25 9.037a.75.75 0 0 1 .75.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 .75-.75Zm4.25.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 1.5 0Z" />
    </g>
  );
}

const stroke = { fill: "none", stroke: "currentColor", strokeWidth: 1.6, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
export const Icon = {
  refresh: (p: { spinning?: boolean }) => (
    <svg viewBox="0 0 16 16" className={p.spinning ? "spin" : undefined} {...stroke}><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" /><path d="M13.5 2.5v3h-3" /></svg>
  ),
  gear: () => (
    <svg viewBox="0 0 16 16" {...stroke} strokeWidth={1.4}><path d="M6.9 1.7h2.2l.4 1.8 1.2.5 1.6-1 1.5 1.5-1 1.6.5 1.2 1.8.4v2.2l-1.8.4-.5 1.2 1 1.6-1.5 1.5-1.6-1-1.2.5-.4 1.8H6.9l-.4-1.8-1.2-.5-1.6 1-1.5-1.5 1-1.6-.5-1.2-1.8-.4V6.9l1.8-.4.5-1.2-1-1.6 1.5-1.5 1.6 1 1.2-.5Z" /><circle cx="8" cy="8" r="2.1" /></svg>
  ),
  info: () => <svg viewBox="0 0 16 16" {...stroke}><circle cx="8" cy="8" r="6.2" /><path d="M8 7.2v4M8 4.9v.1" /></svg>,
  download: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M8 2v8M4.5 6.8 8 10.3l3.5-3.5M2.5 13.5h11" /></svg>,
  list: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h.1M2.5 8h.1M2.5 12h.1" /></svg>,
  chart: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M2 14h12M4 11V7M8 11V3M12 11V8" /></svg>,
  gauge: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M2.5 11a5.5 5.5 0 1 1 11 0" /><path d="m8 11 2.5-3.5" /></svg>,
  calendar: () => <svg viewBox="0 0 16 16" {...stroke}><rect x="2" y="3" width="12" height="11" rx="2" /><path d="M2 6.5h12M5.5 1.5v3M10.5 1.5v3" /></svg>,
  cpu: () => <svg viewBox="0 0 16 16" {...stroke}><rect x="4" y="4" width="8" height="8" rx="1.5" /><path d="M6 1.5v2.5M10 1.5v2.5M6 12v2.5M10 12v2.5M1.5 6H4M1.5 10H4M12 6h2.5M12 10h2.5" /></svg>,
  grid: () => <svg viewBox="0 0 16 16" {...stroke}><rect x="2" y="2" width="5" height="5" rx="1" /><rect x="9" y="2" width="5" height="5" rx="1" /><rect x="2" y="9" width="5" height="5" rx="1" /><rect x="9" y="9" width="5" height="5" rx="1" /></svg>,
  insights: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M1.8 12.5 6 8.2l2.8 2.6 5.4-6" /><path d="M10.6 4.8h3.6v3.6" /></svg>,
  back: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M10 3 5 8l5 5" /></svg>,
  chevron: () => <svg viewBox="0 0 16 16" {...stroke}><path d="m4 6 4 4 4-4" /></svg>,
  expand: () => <svg viewBox="0 0 16 16" {...stroke}><path d="M9.5 2.5h4v4M13.5 2.5 8.5 7.5M6.5 13.5h-4v-4M2.5 13.5l5-5" /></svg>,
};

export function radioStep(key: string, index: number, count: number): number | null {
  switch (key) {
    case "ArrowRight":
    case "ArrowDown":
      return (index + 1) % count;
    case "ArrowLeft":
    case "ArrowUp":
      return (index - 1 + count) % count;
    default:
      return null;
  }
}

export function onRadioKeyDown(e: KeyboardEvent<HTMLElement>, index: number, count: number, select: (i: number) => void) {
  const next = radioStep(e.key, index, count);
  if (next == null) return;
  e.preventDefault();
  select(next);
  e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
}

export function Seg<T extends string>({ label, value, options, onChange, className, radio }: {
  label: string; value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void; className?: string; radio?: boolean;
}) {
  const cls = className ? `seg ${className}` : "seg";
  if (!radio) {
    return (
      <span className={cls} role="group" aria-label={label}>
        {options.map(([k, l]) => <button key={k} className={value === k ? "on" : ""} aria-pressed={value === k} onClick={() => onChange(k)}>{l}</button>)}
      </span>
    );
  }
  const index = Math.max(0, options.findIndex(([k]) => k === value));
  return (
    <span className={cls} role="radiogroup" aria-label={label}
      onKeyDown={(e) => onRadioKeyDown(e, index, options.length, (i) => onChange(options[i][0]))}>
      {options.map(([k, l], i) => (
        <button key={k} role="radio" aria-checked={value === k} tabIndex={i === index ? 0 : -1} className={value === k ? "on" : ""} onClick={() => onChange(k)}>{l}</button>
      ))}
    </span>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="switch" aria-label={label}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span />
    </label>
  );
}


type StatusStyle = { title: string; color: string; text: string; icon: string };

export const STATUS: Record<PaceStatus, StatusStyle> = {
  noAllowance: { title: "No Allowance", color: "var(--muted)", text: "var(--muted)", icon: "–" },
  under: { title: "Under", color: "var(--blue)", text: "var(--link)", icon: "↓" },
  onPace: { title: "On Pace", color: "var(--green)", text: "var(--green-text)", icon: "✓" },
  ahead: { title: "Ahead", color: "var(--orange)", text: "var(--orange-text)", icon: "↑" },
  exhausted: { title: "Exhausted", color: "var(--red)", text: "var(--red-text)", icon: "!" },
};
export const tint = (c: string, p: number) => `color-mix(in srgb, ${c} ${p}%, transparent)`;

export function statusView(snap: Snapshot): StatusStyle {
  const st = snap.pace.status, base = STATUS[st];
  if (st === "exhausted" && snap.allowanceSource === "custom") return { ...base, title: "Allowance reached" };
  return base;
}

export function StatusChip({ snap }: { snap: Snapshot }) {
  const s = statusView(snap);
  return <span className="chip" style={{ background: tint(s.color, 16), color: s.text }}>{s.icon} {s.title}</span>;
}

export function statusMessage(snap: Snapshot): ReactNode {
  const p = snap.pace;
  const reset = f.resetDay(p.cycle.end);
  const resetIn = f.plural(Math.ceil(p.remainingDays), "day");
  const daily = p.dailyBudget ?? 0;
  const allowance = p.allowance ?? 0;
  const entitlement = snap.githubPlan?.entitlement ?? 0;
  if (snap.copilotStopped) return <>Your whole Entitlement is used. Your plan doesn't permit Overage, so Copilot has <b>stopped</b> until it resets in <b>{resetIn}</b> ({reset}).</>;
  const overage = snap.overage > 0 ? <> <b>{f.aic(snap.overage)} AIC</b> of Overage so far, billed separately.</> : null;
  if (p.status === "exhausted" && snap.entitlementUsedUp) return <>Your whole Entitlement is used. Usage from here is billed as Overage.{overage} Resets in <b>{resetIn}</b> ({reset}).</>;
  switch (p.status) {
    case "noAllowance":
      return <>No Allowance yet. Set one in Settings.{overage}</>;
    case "under":
      return <><b>{f.short(-(p.delta ?? 0))} AIC</b> under Even Pace. Daily Budget: <b>{f.short(daily)} AIC</b>.{overage}</>;
    case "onPace":
      return <>Daily Budget: <b>{f.short(daily)} AIC</b>.{overage}</>;
    case "ahead":
      return p.runOutAt
        ? <>At {f.short(p.avgDaily)} AIC a day, runs out on <b>{f.monthDay(p.runOutAt)}</b>. Daily Budget: <b>{f.short(daily)} AIC</b>.{overage}</>
        : <><b>{f.short(p.delta ?? 0)} AIC</b> ahead of Even Pace. Daily Budget: <b>{f.short(daily)} AIC</b>.{overage}</>;
    case "exhausted":
      if (snap.githubPlan?.unlimited) return <>Your Allowance is exhausted · <b>{f.aic(p.used)}</b> of {f.aic(allowance)} AIC. Your plan is unlimited, so Copilot keeps working. Resets in <b>{resetIn}</b> ({reset}).</>;
      if (snap.githubPlan && entitlement > 0) return <>Your Allowance is exhausted · <b>{f.aic(p.used)}</b> of {f.aic(allowance)} AIC. Copilot keeps working until your Entitlement ({f.aic(entitlement)} AIC) runs out. Resets in <b>{resetIn}</b> ({reset}).</>;
      return <>Allowance exhausted · <b>{f.aic(p.used)}</b> of {f.aic(allowance)} AIC. Resets in <b>{resetIn}</b> ({reset}).</>;
  }
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

export function PaceBar({ pace, height = 8 }: { pace: Pace; height?: number }) {
  const used = clamp01(pace.fracUsed ?? 0);
  return (
    <div className="bar" style={{ height }} role="progressbar" aria-label="Share of the Allowance used" aria-valuetext={`${f.pct(pace.fracUsed ?? 0)} used; Even Pace today is ${f.pct(pace.fracElapsed)}`}
      aria-valuenow={Math.round(used * 100)} aria-valuemin={0} aria-valuemax={100}>
      <div className="fill" style={{ width: `${used * 100}%`, background: STATUS[pace.status].color }} />
      <div className="tick" style={{ left: `${pace.fracElapsed * 100}%` }} title="Even Pace today" />
    </div>
  );
}

export function Amount({ big, total, size, color }: { big: string; total?: string; size: number; color?: string }) {
  return (
    <>
      <span className="num" style={{ fontSize: size, fontWeight: 700, color, whiteSpace: "nowrap" }}>{big}</span>
      {total && <span className="num faint" style={{ fontSize: Math.round(size * 0.55), fontWeight: 500, whiteSpace: "nowrap" }}>{total}</span>}
    </>
  );
}

export function CycleSummary({ pace, mode, size = 28, capped }: { pace: Pace; mode?: Settings["cycleHeadline"]; size?: number; capped?: boolean }) {
  const head = headline(pace, mode);
  const hover = usePaceHover(pace, capped);
  return (
    <div {...hover.props} style={{ position: "relative" }}>
      {hover.el}
      <div className="row" style={{ alignItems: "baseline", gap: 5 }}>
        <Amount big={head.big} total={head.total} size={size} />
        <span className="muted" style={{ fontSize: 12.5, whiteSpace: "nowrap" }}>{head.sub}</span>
      </div>
      {pace.allowance != null && <div style={{ margin: "8px 0 6px" }}><PaceBar pace={pace} /></div>}
    </div>
  );
}

export function CycleSummaryView({ snap }: { snap: Snapshot }) {
  const { pace, settings, allowanceCapped } = snap;
  return settings.cycleSummary === "bar" && pace.allowance != null
    ? <div style={{ margin: "6px 0 2px" }}><CycleSummary pace={pace} mode={settings.cycleHeadline} capped={allowanceCapped} /></div>
    : <Gauge pace={pace} mode={settings.cycleHeadline} capped={allowanceCapped} />;
}

export function TodayMeter({ pace, today, compact }: { pace: Pace; today: number; compact?: boolean }) {
  const daily = pace.dailyBudget;
  const size = compact ? 17 : 21;
  if (daily == null || daily <= 0) {
    return (
      <div className="row" style={{ alignItems: "baseline" }}>
        <span className="num" style={{ fontSize: size, fontWeight: 700 }}>{f.aic(today)}</span>
        <span className="muted">AIC today</span><span className="grow" />
        {pace.status === "exhausted" && <span className="faint" style={{ fontSize: 11.5 }}>beyond your Allowance</span>}
      </div>
    );
  }
  const over = today > daily;
  const col = over ? "var(--orange)" : "var(--green)";
  const textCol = over ? "var(--orange-text)" : "var(--green-text)";
  const bar = (
    <div className="bar" style={{ marginTop: 7 }}>
      <div className="fill" style={{ width: `${Math.min(100, (today / daily) * 100)}%`, background: col }} />
    </div>
  );
  const total = (v: string) => <span className="num faint" style={{ fontSize: Math.round(size * 0.55), fontWeight: 500, whiteSpace: "nowrap" }}>/ {v} AIC</span>;
  const side = (s: string, c?: string) => <span className="num" style={{ fontSize: 11.5, whiteSpace: "nowrap", color: c ?? "var(--muted)" }}>{s}</span>;
  return (
    <>
      <div className="row" style={{ alignItems: "baseline", gap: 5 }}>
        <span className="num" style={{ fontSize: size, fontWeight: 700 }}>{f.aic(today)}</span>
        {total(f.short(daily))}<span className="grow" />
        {over ? side(`${f.short(today - daily)} over`, textCol) : side(`${f.short(daily - today)} left`)}
      </div>
      {bar}
    </>
  );
}


export function headline(pace: Pace, mode: Settings["cycleHeadline"] = "used"): { big: string; total?: string; sub: string } {
  const b = pace.allowance;
  if (b == null) return { big: f.aic(pace.used), sub: "AIC this Cycle · no Allowance" };
  if (mode === "remaining") {
    const left = Math.round((b - pace.used) * 100) / 100;
    return left >= 0
      ? { big: f.aic(left), total: `/ ${f.aic(b)}`, sub: `AIC left · ${f.aic(pace.used)} used` }
      : { big: f.aic(-left), total: `/ ${f.aic(b)}`, sub: `AIC over · ${f.aic(pace.used)} used` };
  }
  return { big: f.aic(pace.used), total: `/ ${f.aic(b)}`, sub: `AIC · ${f.pct(pace.fracUsed ?? 0)} used` };
}

function gaugeDetails(p: Pace, capped?: boolean): ReactNode {
  const row = (k: string, v: ReactNode, c?: string) => (
    <div className="row" style={{ gap: 12 }}><span className="muted grow">{k}</span><b className="num" style={{ color: c }}>{v}</b></div>
  );
  const reset = `${f.resetDay(p.cycle.end)} · ${f.plural(Math.ceil(p.remainingDays), "day")}`;
  if (p.allowance == null) {
    return <div style={{ display: "grid", gap: 3 }}>{row("Used this Cycle", `${f.aic(p.used)} AIC`)}{row("Avg / day", f.aic(p.avgDaily))}{row("Resets", reset)}</div>;
  }
  const left = p.allowance - p.used;
  const ahead = (p.delta ?? 0) > 0;
  return (
    <div style={{ display: "grid", gap: 3, minWidth: 190 }}>
      {row("Used", `${f.aic(p.used)} of ${f.aic(p.allowance)} AIC`)}
      {capped && <div className="faint" style={{ fontSize: 11.5 }}>{CAPPED_NOTE}</div>}
      {row(left >= 0 ? "Left" : "Over", `${f.aic(Math.abs(left))} AIC`, left < 0 ? "var(--red-text)" : undefined)}
      {row("Even Pace today", `${f.aic(p.evenPace ?? 0)} AIC`)}
      {p.delta != null && row(ahead ? "Ahead of Even Pace by" : "Under Even Pace by", `${f.short(Math.abs(p.delta))} AIC`, ahead ? "var(--orange-text)" : undefined)}
      {p.status !== "exhausted" && p.dailyBudget != null && row("Daily Budget", `${f.short(p.dailyBudget)} AIC`)}
      {p.runOutAt ? row("Runs out", f.monthDay(p.runOutAt), "var(--orange-text)") : row("Projected", `${f.aic(p.projected)} AIC`)}
      {row("Resets", reset)}
    </div>
  );
}

export const CAPPED_NOTE = "Allowance capped at your Entitlement: your plan doesn't permit Overage, so Copilot stops there.";
export const UNLOGGED_NOTE = "Unlogged Usage: AIC GitHub counted that the logs on this Mac don't show, such as github.com, mobile, another machine or the cloud coding agent.";

const gaugeSummary = (p: Pace) =>
  p.allowance == null
    ? `${f.aic(p.used)} AIC used this Cycle`
    : `${f.aic(p.used)} of ${f.aic(p.allowance)} AIC used. Even Pace today is ${f.aic(p.evenPace ?? 0)}.${p.runOutAt ? ` May run out ${f.monthDay(p.runOutAt)}.` : ""} Resets ${f.resetDay(p.cycle.end)}.`;

function usePaceHover(pace: Pace, capped?: boolean) {
  const tip = useTip();
  const ref = useRef<HTMLDivElement>(null);
  const showAt = (x: number, y: number) => tip.showAt(x, y, gaugeDetails(pace, capped));
  return {
    el: tip.el,
    props: {
      ref, tabIndex: 0, className: "gauge-hit", role: "img", "aria-label": gaugeSummary(pace),
      onMouseMove: (e: React.MouseEvent) => showAt(e.clientX, e.clientY), onMouseLeave: tip.hide,
      onFocus: () => { const r = ref.current!.getBoundingClientRect(); showAt(r.left + 40, r.top + 40); }, onBlur: tip.hide,
    },
  };
}

const GAUGE = {
  width: 288, height: 146, viewX: -4, radius: 98, centreX: 140, centreY: 118, stroke: 16, tickInset: 13,
  bigOffset: -26, subOffset: -6, footOffset: 14, totalScale: 0.47, minArcFraction: 0.003,
};
const BIG_NUMBER = { minSize: 20, maxSize: 36, budget: 160, bigCharWidth: 0.6, totalCharWidth: 0.28, totalGap: 0.2 };

export function bigNumberSize(big: string, total?: string): number {
  const totalWidth = total ? total.length * BIG_NUMBER.totalCharWidth + BIG_NUMBER.totalGap : 0;
  const width = big.length * BIG_NUMBER.bigCharWidth + totalWidth;
  return Math.max(BIG_NUMBER.minSize, Math.min(BIG_NUMBER.maxSize, BIG_NUMBER.budget / width));
}

export function Gauge({ pace, mode, capped }: { pace: Pace; mode?: Settings["cycleHeadline"]; capped?: boolean }) {
  const { radius: R, centreX: cx, centreY: cy, stroke: sw } = GAUGE;
  const pt = (fr: number, r = R) => { const a = Math.PI * (1 - clamp01(fr)); return [cx + r * Math.cos(a), cy - r * Math.sin(a)]; };
  const arc = (fr: number) => { const [x, y] = pt(fr); return `M ${cx - R} ${cy} A ${R} ${R} 0 0 1 ${x.toFixed(1)} ${y.toFixed(1)}`; };
  const col = STATUS[pace.status].color;
  const fe = pace.fracElapsed;
  const [t1x, t1y] = pt(fe, R - GAUGE.tickInset), [t2x, t2y] = pt(fe, R + GAUGE.tickInset);
  const hover = usePaceHover(pace, capped);
  const used = pace.fracUsed ?? 0;
  const head = headline(pace, mode);
  const bigSize = bigNumberSize(head.big, head.total);
  return (
    <div {...hover.props}>
    <svg width={GAUGE.width} height={GAUGE.height} viewBox={`${GAUGE.viewX} 0 ${GAUGE.width} ${GAUGE.height}`} style={{ display: "block", margin: "2px auto 0", overflow: "visible" }} aria-hidden>
      <path d={arc(1)} fill="none" stroke="var(--track)" strokeWidth={sw} strokeLinecap="round" />
      {used > GAUGE.minArcFraction && <path d={arc(used)} fill="none" stroke={col} strokeWidth={sw} strokeLinecap="round" />}
      {pace.allowance != null && (
        <>
          <line x1={t1x} y1={t1y} x2={t2x} y2={t2y} stroke="var(--text)" strokeWidth={2.5} strokeLinecap="round" />
        </>
      )}
      <text x={cx} y={cy + GAUGE.bigOffset} textAnchor="middle" fontSize={bigSize} fontWeight={700} fill="var(--text)" className="num">
        {head.big}
        {head.total && <tspan dx={4} fontSize={Math.round(bigSize * GAUGE.totalScale)} fontWeight={500} fill="var(--faint)">{head.total}</tspan>}
      </text>
      <text x={cx} y={cy + GAUGE.subOffset} textAnchor="middle" fontSize={12} fill="var(--muted)">{head.sub}</text>
      <text x={cx} y={cy + GAUGE.footOffset} textAnchor="middle" fontSize={10.5} fill="var(--faint)">
        resets in {f.plural(Math.ceil(pace.remainingDays), "day")} · {f.resetDay(pace.cycle.end)}
      </text>
    </svg>
    {hover.el}
    </div>
  );
}


const PALETTE = ["var(--m1)", "var(--m2)", "var(--m3)", "var(--m4)", "var(--m5)", "var(--m6)"];
export function colorMap(snap: Snapshot): (g: string) => string {
  const t = new Map<string, number>();
  for (const r of snap.records) t.set(r.model, (t.get(r.model) ?? 0) + (r.aic ?? 0) * 1e6 + r.tokens.input + r.tokens.output);
  const ranked = [...t].sort((a, b) => b[1] - a[1]).map(([k]) => k);
  return (g) => (g === "Other" ? "var(--m7)" : PALETTE[ranked.indexOf(g) >= 0 ? ranked.indexOf(g) % PALETTE.length : Math.abs(hash(g)) % PALETTE.length]);
}
const hash = (s: string) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7);


export function useTip() {
  const [tip, setTip] = useState<{ x: number; y: number; body: ReactNode } | null>(null);
  const el = tip ? (
    <div className="chart-tip" style={{ left: Math.min(tip.x + 14, window.innerWidth - 220), top: tip.y + 14 }}>{tip.body}</div>
  ) : null;
  return {
    el,
    show: (e: React.MouseEvent, body: ReactNode) => setTip({ x: e.clientX, y: e.clientY, body }),
    showAt: (x: number, y: number, body: ReactNode) => setTip({ x, y, body }),
    hide: () => setTip(null),
  };
}


export function useWidth<T extends HTMLElement>(fallback: number) {
  const ref = useRef<T>(null);
  const [w, setW] = useState(fallback);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setW(el.clientWidth || fallback));
    ro.observe(el);
    return () => ro.disconnect();
  }, [fallback]);
  return [ref, w] as const;
}

export function StackedBars(props: {
  days: DayStack[]; groups: string[]; metric: Metric; color: (g: string) => string; height?: number; width?: number;
  evenPerDay?: number | null; todayKey?: string; futureUntil?: number; dim?: Dim | null; onDay?: (key: string) => void; selected?: string;
  evenLabel?: boolean;
}) {
  const { days, metric, color, height = 160, evenPerDay, todayKey } = props;
  const tip = useTip();
  const future: number[] = [];
  if (props.futureUntil) {
    for (let t = addDays(days.length ? days[days.length - 1].day : 0, 1); t < props.futureUntil; t = addDays(t, 1)) future.push(t);
  }
  const n = days.length + future.length;
  const [box, W] = useWidth<HTMLDivElement>(props.width ?? 600);
  const pad = { l: 34, b: 18, t: 6 };
  const innerW = W - pad.l, innerH = height - pad.b - pad.t;
  const scale = f.niceScale(Math.max(...days.map((d) => d.total), evenPerDay ?? 0, 1e-9), 4);
  const max = scale.max;
  const bw = innerW / Math.max(n, 1);
  const y = (v: number) => pad.t + innerH - (v / max) * innerH;
  const ticks = Array.from({ length: Math.round(max / scale.step) + 1 }, (_, i) => i * scale.step);
  const labelEvery = n <= 10 ? 1 : n <= 35 ? 7 : n <= 100 ? 14 : 30;
  return (
    <div ref={box} className="measure" style={{ position: "relative" }}>
      <svg viewBox={`0 0 ${W} ${height}`} width={W} height={height} style={{ display: "block", overflow: "visible" }} role="img"
        aria-label={(() => {
          const tot = days.reduce((s, d) => s + d.total, 0);
          const peak = days.reduce((b, d) => (d.total > b.total ? d : b), days[0] ?? { total: 0, day: 0 } as DayStack);
          return `${days.length} days, total ${f.metricValue(tot, metric)}` + (peak && peak.total ? `, highest ${f.metricValue(peak.total, metric)} on ${f.weekdayDate(peak.day)}` : "");
        })()}>
        {ticks.map((v, i) => (
          <g key={i}>
            <line x1={pad.l} x2={W} y1={y(v)} y2={y(v)} stroke="var(--line)" strokeDasharray={i ? "2 3" : undefined} />
            <text x={pad.l - 6} y={y(v) + 3.5} textAnchor="end" fontSize={10} fill="var(--faint)">{f.axisValue(v, scale.step, metric)}</text>
          </g>
        ))}
        {days.map((d, i) => {
          let acc = 0;
          const x = pad.l + i * bw + Math.min(1.5, bw * 0.15);
          const w = Math.max(1, bw - Math.min(3, bw * 0.3));
          const isToday = d.key === todayKey;
          return (
            <g key={d.key} onMouseMove={(e) => tip.show(e, <DayTip d={d} metric={metric} color={color} dim={props.dim ?? null} />)} onMouseLeave={tip.hide}
              onClick={() => props.onDay?.(d.key)} style={{ cursor: props.onDay ? "pointer" : "default" }}>
              <rect x={pad.l + i * bw} y={pad.t} width={bw} height={innerH} fill={props.selected === d.key ? "var(--accent-soft)" : "transparent"} />
              {d.parts.map((p) => {
                if (!p.value) return null;
                const h = (p.value / max) * innerH;
                acc += h;
                return <rect key={p.group} x={x} y={pad.t + innerH - acc} width={w} height={h} fill={color(p.group)} rx={Math.min(2, w / 3)} />;
              })}
              {(i % labelEvery === 0 || isToday) && (
                <text x={x + w / 2} y={height - 4} textAnchor="middle" fontSize={9.5} fill={isToday ? "var(--text)" : "var(--faint)"}>
                  {isToday ? "today" : f.monthDay(d.day)}
                </text>
              )}
            </g>
          );
        })}
        {future.map((t, j) => (
          <rect key={t} x={pad.l + (days.length + j) * bw + 1.5} y={pad.t + innerH - 3} width={Math.max(1, bw - 3)} height={3} rx={1} fill="var(--track)" />
        ))}
        {evenPerDay != null && evenPerDay > 0 && (
          <g>
            <line x1={pad.l} x2={W} y1={y(evenPerDay)} y2={y(evenPerDay)} stroke="var(--muted)" strokeDasharray="4 3" />
            {props.evenLabel !== false && <text x={pad.l + 4} y={y(evenPerDay) - 4} fontSize={9.5} fill="var(--muted)">even {f.short(evenPerDay)}/day</text>}
          </g>
        )}
      </svg>
      {tip.el}
    </div>
  );
}

function DayTip({ d, metric, color, dim }: { d: DayStack; metric: Metric; color: (g: string) => string; dim: Dim | null }) {
  return (
    <div>
      <div className="row" style={{ marginBottom: 4 }}><b>{f.weekdayDate(d.day)}</b><span className="grow" /><b className="num">{f.metricValue(d.total, metric)}</b></div>
      {d.parts.filter((p) => p.value > 0).sort((a, b) => b.value - a.value).map((p) => (
        <div key={p.group} className="row" style={{ gap: 6 }}>
          <span className="swatch" style={{ background: color(p.group) }} />
          <span className="grow">{dim ? groupLabel(p.group, dim) : p.group}</span>
          <span className="num muted">{f.metricValue(p.value, metric)}</span>
        </div>
      ))}
    </div>
  );
}


export function menuBarOf(s: Partial<Pick<Settings, "menuBarDisplay" | "menuBarIcon" | "menuBarNumber">>): MenuBarLook {
  return { display: s.menuBarDisplay || "icon-number", icon: s.menuBarIcon || "disc", number: s.menuBarNumber || "pct" };
}

export const MENU_BAR_ICONS: { key: MenuBarIcon; title: string }[] = [
  { key: "disc", title: "Disc" },
  { key: "ring", title: "Ring" },
  { key: "copilot", title: "Copilot" },
  { key: "progress", title: "Progress" },
];

export const MENU_BAR_NUMBERS: { key: MenuBarNumber; title: string }[] = [
  { key: "pct", title: "Percent used" },
  { key: "used", title: "AIC used" },
  { key: "left", title: "AIC left" },
  { key: "used-total", title: "Used / Allowance" },
];

export function menuBarNumberText(n: MenuBarNumber, pace: Pace): string {
  const b = pace.allowance, used = f.aic(pace.used);
  if (n === "used" || b == null) return n === "used-total" && b == null ? `${used} AIC` : used;
  if (n === "left") return pace.used <= b ? `${f.aic(b - pace.used)} left` : `${f.aic(pace.used - b)} over`;
  if (n === "used-total") return `${used} / ${f.aic(b)}`;
  return f.pct(pace.fracUsed ?? 0);
}

export function MenuBarIconMark({ icon, pace, alert, size = 16 }: { icon: MenuBarIcon; pace: Pace; alert?: string; size?: number }) {
  const maskId = "cut" + useId().replace(/[^a-zA-Z0-9]/g, "");
  const col = alert ?? "currentColor";
  const fr = pace.fracUsed != null ? clamp01(pace.fracUsed) : null;
  const g = (s: number, fill: string) => <g transform={`translate(${8 - 8 * s} ${8 - 8 * s}) scale(${s})`} style={{ color: fill }}><CopilotGlyphPaths /></g>;
  let mark: React.JSX.Element;
  if (icon === "disc") {
    const a = 2 * Math.PI * (fr ?? 0) - Math.PI / 2;
    const wedge = fr == null || fr <= 0.004 ? null
      : fr >= 0.999 ? <circle cx="8" cy="8" r="8" />
      : <path d={`M8 8 L8 0 A8 8 0 ${fr > 0.5 ? 1 : 0} 1 ${8 + 8 * Math.cos(a)} ${8 + 8 * Math.sin(a)} Z`} />;
    mark = (
      <>
        <defs><mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width="16" height="16"><rect width="16" height="16" fill="#fff" />{g(0.75, "#000")}</mask></defs>
        <g mask={`url(#${maskId})`} fill={col}><circle cx="8" cy="8" r="8" fillOpacity={fr == null ? 1 : 0.4} />{wedge}</g>
      </>
    );
  } else if (icon === "ring") {
    const C = 2 * Math.PI * 7.1;
    mark = (
      <>
        <circle cx="8" cy="8" r="7.1" fill="none" stroke={col} strokeOpacity={fr == null ? 1 : 0.45} strokeWidth={1.8} />
        {fr != null && fr > 0.004 && <circle cx="8" cy="8" r="7.1" fill="none" stroke={col} strokeWidth={1.8} strokeLinecap="round" strokeDasharray={`${C * fr} ${C}`} transform="rotate(-90 8 8)" />}
        {g(0.6, "currentColor")}
      </>
    );
  } else if (icon === "copilot") {
    mark = alert ? (
      <>
        <defs><mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width="16" height="16"><rect width="16" height="16" fill="#fff" /><circle cx="12.6" cy="12.6" r="4.1" fill="#000" /></mask></defs>
        <g mask={`url(#${maskId})`}>{g(1, "currentColor")}</g>
        <circle cx="12.6" cy="12.6" r="3.1" fill={alert} />
      </>
    ) : g(1, "currentColor");
  } else {
    const C = 2 * Math.PI * 6.2, a = 2 * Math.PI * pace.fracElapsed - Math.PI / 2;
    mark = (
      <>
        <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeOpacity={0.3} strokeWidth={2.4} />
        <circle cx="8" cy="8" r="6.2" fill="none" stroke={col} strokeWidth={2.4} strokeDasharray={`${C * (fr ?? 0)} ${C}`} transform="rotate(-90 8 8)" />
        <line x1={8 + 3.2 * Math.cos(a)} y1={8 + 3.2 * Math.sin(a)} x2={8 + 7.9 * Math.cos(a)} y2={8 + 7.9 * Math.sin(a)} stroke="currentColor" strokeWidth={1.2} strokeLinecap="round" />
      </>
    );
  }
  return <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden style={{ flex: "none" }}>{mark}</svg>;
}

export function menuBarAlertColour(status: PaceStatus, alertColors: boolean): string | undefined {
  if (!alertColors) return undefined;
  switch (status) {
    case "under":
    case "onPace":
      return "var(--green)";
    case "ahead":
      return "var(--orange)";
    case "exhausted":
      return "var(--red)";
    case "noAllowance":
      return undefined;
  }
}

export function MenuBarPreview({ look, pace, alertColors }: { look: MenuBarLook; pace: Pace; alertColors: boolean }) {
  const alert = look.display !== "number" ? menuBarAlertColour(pace.status, alertColors) : undefined;
  return (
    <span className="menubar-preview" aria-hidden>
      {look.display !== "number" && <MenuBarIconMark icon={look.icon} pace={pace} alert={alert} />}
      {look.display !== "icon" && <span className="num">{menuBarNumberText(look.number, pace)}</span>}
    </span>
  );
}

