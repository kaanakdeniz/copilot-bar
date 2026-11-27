import { useEffect, useMemo, useRef, useState } from "react";
import { MENU_BAR_ICONS, MENU_BAR_NUMBERS, menuBarOf, Seg } from "../components";
import type { Settings } from "../types";
import type { StudioConfig } from "./browser";

type ThemeView = "light" | "dark" | "both";
type ModeView = "overview" | "expanded" | "calendar" | "all";
interface Cfg extends StudioConfig { themeView: ThemeView; modeView: ModeView; allowanceFrom: "github" | "personal" }
interface Candidate { name: string; cfg: Cfg; savedAt: number }

const ORIGINAL: Cfg = {
  data: { used: 137, allowance: 200, day: 21, today: 4, localShare: 1, overagePermitted: true, connection: "ok", history: "steady" },
  settings: { menuBarDisplay: "icon-number", menuBarIcon: "disc", menuBarNumber: "pct", alertColors: true, textScale: 1, palette: "default" },
  themeView: "both",
  modeView: "overview",
  allowanceFrom: "github",
};

const PRESETS: { name: string; data: Partial<Cfg["data"]> }[] = [
  { name: "On pace", data: { used: 137, allowance: 200, day: 21, today: 4, localShare: 1, connection: "ok" } },
  { name: "Ahead", data: { used: 176, allowance: 200, day: 21, today: 9, localShare: 1, connection: "ok" } },
  { name: "Under", data: { used: 58, allowance: 200, day: 21, today: 1, localShare: 1, connection: "ok" } },
  { name: "Exhausted", data: { used: 206, allowance: 200, day: 21, today: 6, localShare: 1, connection: "ok" } },
  { name: "Day 1", data: { used: 2.4, allowance: 200, day: 1, today: 2.4, localShare: 1, connection: "ok" } },
  { name: "No Allowance", data: { used: 1.73, allowance: 0, day: 21, today: 0, localShare: 1, connection: "ok" } },
  { name: "Mostly web usage", data: { used: 120, allowance: 200, day: 21, today: 1, localShare: 0.25, connection: "ok" } },
  { name: "Offline", data: { used: 90, allowance: 200, day: 21, today: 3, localShare: 1, connection: "offline" } },
  { name: "Fresh reset", data: { used: 0, allowance: 200, day: 1, today: 0, localShare: 1, connection: "ok" } },
  { name: "Quiet Cycle, pricey today", data: { used: 100, allowance: 200, day: 21, today: 20, localShare: 1, connection: "ok" } },
  { name: "Heavy: 100–1k/day", data: { used: 9800, allowance: 15000, day: 21, today: 640, localShare: 1, connection: "ok", history: "steady" } },
  { name: "1,000 in one day", data: { used: 7400, allowance: 15000, day: 21, today: 1000, localShare: 1, connection: "ok", history: "steady" } },
  { name: "Looks exhausted", data: { used: 199.6, allowance: 200, day: 30, today: 0.6, localShare: 1, connection: "ok" } },
];

const KEY = "copilotbar.studio.v1";
function load<T>(k: string, d: T): T {
  try { const v = localStorage.getItem(k); return v ? { ...d, ...JSON.parse(v) } : d; } catch { return d; }
}
function store(k: string, v: unknown) {
  try { localStorage.setItem(k, JSON.stringify(v)); } catch {}
}
const merge = (c: Cfg): Cfg => ({ ...ORIGINAL, ...c, data: { ...ORIGINAL.data, ...c.data }, settings: { ...ORIGINAL.settings, ...c.settings } });

export default function Studio() {
  const [cfg, setCfg] = useState<Cfg>(() => merge(load(KEY + ".cfg", ORIGINAL)));
  const [cands, setCands] = useState<Candidate[]>(() => load<{ list: Candidate[] }>(KEY + ".cands", { list: [] }).list);
  const [pinned, setPinned] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => store(KEY + ".cfg", cfg), [cfg]);
  useEffect(() => store(KEY + ".cands", { list: cands }), [cands]);
  useEffect(() => { document.title = "CopilotBar Studio"; }, []);

  const data = (patch: Partial<Cfg["data"]>) => setCfg((c) => ({ ...c, data: { ...c.data, ...patch } }));
  const settings = (patch: Partial<Cfg["settings"]>) => setCfg((c) => ({ ...c, settings: { ...c.settings, ...patch } }));
  useEffect(() => {
    const onMsg = (e: MessageEvent) => {
      if (e.origin === location.origin && e.data?.type === "studio-settings") settings(e.data.patch as Partial<Cfg["settings"]>);
    };
    window.addEventListener("message", onMsg);
    return () => window.removeEventListener("message", onMsg);
  }, []);

  const pin = pinned != null ? cands[pinned] : null;
  const columns = [{ label: "Current", cfg }, ...(pin ? [{ label: `Candidate ${pin.name}`, cfg: merge(pin.cfg) }] : [])];

  const save = () => {
    const name = String.fromCharCode(65 + (cands.length % 26)) + (cands.length >= 26 ? Math.floor(cands.length / 26) : "");
    setCands([...cands, { name, cfg, savedAt: Date.now() }]);
  };
  const copy = async () => {
    const spec = { settings: cfg.settings, data: cfg.data };
    try { await navigator.clipboard.writeText(JSON.stringify(spec, null, 2)); setCopied(true); setTimeout(() => setCopied(false), 1500); } catch {}
  };
  const d = cfg.data, s = cfg.settings;
  const usedMax = Math.max(300, d.allowance * 1.3);

  return (
    <div className="studio">
      <style>{STUDIO_CSS}</style>
      <aside className="studio-side">
        <div className="row" style={{ marginBottom: 4 }}><b style={{ fontSize: 15 }}>CopilotBar Studio</b><span className="grow" /><button className="btn" onClick={() => { setCfg(ORIGINAL); setPinned(null); }} title="Shipped design and On Pace Cycle">Reset</button></div>

        <Section title="Cycle">
          <div className="chips">{PRESETS.map((p) => <button key={p.name} className="btn" onClick={() => data(p.data)}>{p.name}</button>)}</div>
          <Slider label="Used" value={d.used} min={0} max={usedMax} step={d.allowance > 2000 ? 10 : 0.5} unit="AIC" onChange={(v) => data({ used: v, today: Math.min(d.today, v) })} />
          <Slider label="Allowance" value={d.allowance} min={0} max={20000} step={10} unit="AIC" fmt={(v) => (v === 0 ? "none" : String(v))} onChange={(v) => data({ allowance: v })} />
          <Slider label="Day of Cycle" value={d.day} min={1} max={30} step={1} unit="of 30" onChange={(v) => data({ day: v })} />
          <Slider label="Used today" value={d.today} min={0} max={Math.max(1, Math.min(1500, d.used))} step={d.used > 200 ? 1 : 0.1} unit="AIC" onChange={(v) => data({ today: v })} />
          <Slider label="Found in local logs" value={Math.round(d.localShare * 100)} min={0} max={100} step={5} unit="%" onChange={(v) => data({ localShare: v / 100 })} />
          <Choice label="Usage history" value={d.history ?? "steady"} options={[["steady", "Steady"], ["peak", "One heavy day"], ["recent", "Last week heavy"]]} onChange={(v) => data({ history: v })} />
          <Choice label="GitHub" value={d.connection} options={[["ok", "Fresh"], ["connecting", "Connecting"], ["stale", "Stale"], ["offline", "Logs only"]]} onChange={(v) => data({ connection: v })} />
          <Choice label="Allowance from" value={cfg.allowanceFrom ?? "github"} options={[["github", "GitHub plan"], ["personal", "Own Allowance"]]} onChange={(v) => setCfg((c) => ({ ...c, allowanceFrom: v }))} />
          <Choice label="Overage permitted" value={d.overagePermitted ? "on" : "off"} options={[["on", "Yes"], ["off", "No"]]} onChange={(v) => data({ overagePermitted: v === "on" })} />
        </Section>

        <Section title="Popover">
          <Choice label="Cycle summary (setting)" value={s.cycleSummary ?? "gauge"} options={[["gauge", "Gauge"], ["bar", "Number + bar"]]} onChange={(v) => settings({ cycleSummary: v })} />
          <Choice label="Big number (setting)" value={s.cycleHeadline ?? "used"} options={[["used", "Used"], ["remaining", "Left"]]} onChange={(v) => settings({ cycleHeadline: v })} />
        </Section>

        <Section title="View">
          <Choice label="Popover" value={cfg.modeView} options={[["overview", "Overview"], ["expanded", "Expanded"], ["calendar", "Calendar"], ["all", "All"]]} onChange={(v) => setCfg((c) => ({ ...c, modeView: v }))} />
          <Choice label="Theme" value={cfg.themeView} options={[["light", "Light"], ["dark", "Dark"], ["both", "Both"]]} onChange={(v) => setCfg((c) => ({ ...c, themeView: v }))} />
          {(() => {
            const mb = menuBarOf(s);
            const setMb = (p: Partial<typeof mb>) => { const n = { ...mb, ...p }; settings({ menuBarDisplay: n.display, menuBarIcon: n.icon, menuBarNumber: n.number }); };
            return (
              <>
                <Choice label="Menu bar" value={mb.display} options={[["icon-number", "Icon + number"], ["icon", "Icon"], ["number", "Number"]]} onChange={(v) => setMb({ display: v })} />
                <Choice label="Menu bar icon" value={mb.icon} options={MENU_BAR_ICONS.map((o) => [o.key, o.title] as [typeof o.key, string])} onChange={(v) => setMb({ icon: v })} />
                <label className="ctl"><span>Menu bar number</span>
                  <select className="field" value={mb.number} onChange={(e) => setMb({ number: e.target.value as typeof mb.number })}>
                    {MENU_BAR_NUMBERS.map((o) => <option key={o.key} value={o.key}>{o.title}</option>)}
                  </select>
                </label>
              </>
            );
          })()}
          <Choice label="Alert colours" value={s.alertColors ? "on" : "off"} options={[["on", "On"], ["off", "Off"]]} onChange={(v) => settings({ alertColors: v === "on" })} />
          <Choice label="Text size" value={String(s.textScale ?? 1)} options={[["1", "100%"], ["1.15", "115%"], ["1.3", "130%"]]} onChange={(v) => settings({ textScale: Number(v) })} />
          <Choice label="Chart colours" value={s.palette ?? "default"} options={[["default", "Default"], ["colorblind", "Colour-blind"]]} onChange={(v) => settings({ palette: v })} />
        </Section>

        <Section title="Candidates">
          <div className="row" style={{ gap: 6 }}>
            <button className="btn primary" onClick={save}>Save current</button>
            <button className="btn" onClick={copy}>{copied ? "Copied" : "Copy spec"}</button>
          </div>
          {cands.length === 0 && <div className="faint" style={{ fontSize: 11.5 }}>Save a version to compare.</div>}
          {cands.map((c, i) => (
            <div key={c.savedAt} className="cand row">
              <b style={{ width: 18 }}>{c.name}</b>
              <span className="grow faint" style={{ fontSize: 11 }}>{summary(c.cfg)}</span>
              <button className="btn" onClick={() => setCfg(merge(c.cfg))}>Load</button>
              <button className={pinned === i ? "btn primary" : "btn"} onClick={() => setPinned(pinned === i ? null : i)}>{pinned === i ? "Comparing" : "Compare"}</button>
              <button className="iconbtn" aria-label={`Delete ${c.name}`} onClick={() => { setCands(cands.filter((_, j) => j !== i)); setPinned(null); }}>×</button>
            </div>
          ))}
        </Section>
      </aside>

      <main className="studio-canvas">
        {columns.map((col) => (
          <section key={col.label} className="studio-col">
            <div className="faint" style={{ fontSize: 11.5, marginBottom: 6 }}>{col.label} · {summary(col.cfg)}</div>
            <div className="studio-frames" style={{ flexDirection: columns.length > 1 ? "column" : "row" }}>
              {themesOf(col.cfg).map((theme) => modesOf(col.cfg).map((mode) => (
                <Frame key={col.label + theme + mode} mode={mode} config={{ data: col.cfg.data, settings: { ...col.cfg.settings, theme, allowanceOverride: col.cfg.allowanceFrom === "personal" ? col.cfg.data.allowance : 0 } }} caption={`${mode} · ${theme}`} />
              )))}
            </div>
          </section>
        ))}
      </main>
    </div>
  );
}

const themesOf = (c: Cfg) => (c.themeView === "both" ? (["light", "dark"] as const) : [c.themeView]);
const modesOf = (c: Cfg): Exclude<ModeView, "all">[] => (c.modeView === "all" ? ["overview", "expanded", "calendar"] : [c.modeView]);
const summary = (c: Cfg) => {
  const changed = Object.entries(c.settings).filter(([k, v]) => v !== ORIGINAL.settings[k as keyof Settings]).map(([k, v]) => `${k}: ${v}`);
  return `${c.data.used}${c.data.allowance ? `/${c.data.allowance}` : ""} AIC · day ${c.data.day} · ${changed.length ? changed.join(", ") : "shipped settings"}`;
};

const FRAME_HEIGHT = 880;

function Frame({ config, mode, caption }: { config: StudioConfig; mode: string; caption: string }) {
  const ref = useRef<HTMLIFrameElement>(null);
  const json = JSON.stringify(config);
  const send = () => ref.current?.contentWindow?.postMessage({ type: "studio-config", config: JSON.parse(json) }, location.origin);
  useEffect(send, [json]);
  useEffect(() => {
    const onMsg = (e: MessageEvent) => { if (e.source === ref.current?.contentWindow && e.data?.type === "studio-ready") send(); };
    window.addEventListener("message", onMsg);
    return () => window.removeEventListener("message", onMsg);
  });
  const height = FRAME_HEIGHT;
  const src = useMemo(() => `?view=popover&studio=1&theme=${config.settings.theme}&mode=${mode}`, [config.settings.theme, mode]);
  return (
    <figure style={{ margin: 0 }}>
      <iframe ref={ref} src={src} title={caption} onLoad={send} style={{ width: 520, height, border: "1px solid var(--line)", borderRadius: 10, display: "block" }} />
      <figcaption className="faint" style={{ fontSize: 11, marginTop: 4 }}>{caption}</figcaption>
    </figure>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return <section className="studio-sec"><div className="caps" style={{ marginBottom: 8 }}>{title}</div><div style={{ display: "grid", gap: 10 }}>{children}</div></section>;
}

function Slider({ label, value, min, max, step, unit, fmt, onChange }: { label: string; value: number; min: number; max: number; step: number; unit?: string; fmt?: (v: number) => string; onChange: (v: number) => void }) {
  return (
    <label className="ctl ctl-slider">
      <span className="row"><span className="grow">{label}</span><span className="num">{fmt ? fmt(value) : value}</span>{unit && (!fmt || value !== 0) && <span className="faint">&nbsp;{unit}</span>}</span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

function Choice<T extends string>(props: { label: string; value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void }) {
  return <div className="ctl"><span>{props.label}</span><Seg {...props} /></div>;
}

const STUDIO_CSS = `
html, body { overflow: auto !important; }
.studio { display: grid; grid-template-columns: 330px 1fr; min-height: 100vh; }
.studio-side { border-right: 1px solid var(--line); padding: 16px; display: grid; gap: 16px; align-content: start; position: sticky; top: 0; height: 100vh; overflow: auto; box-sizing: border-box; background: var(--panel); }
.studio-sec { display: grid; }
.studio-canvas { padding: 16px; display: flex; gap: 28px; align-items: flex-start; overflow-x: auto; }
.studio-col { flex: none; }
.studio-frames { display: flex; flex-wrap: wrap; gap: 14px; max-width: 1080px; }
.ctl { display: grid; gap: 5px; font-size: 12.5px; }
.ctl .seg { justify-self: start; }
.ctl-slider input { width: 100%; }
.chips { display: flex; flex-wrap: wrap; gap: 5px; }
.chips .btn { padding: 3px 8px; font-size: 11.5px; }
.cand { gap: 6px; font-size: 12.5px; }
.cand .btn { padding: 3px 8px; font-size: 11.5px; }
`;
