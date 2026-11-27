import { useEffect, useState, type ReactNode } from "react";
import { version } from "../package.json";
import { api, inTauri } from "./backend";
import { CopilotIcon, CycleSummaryView, MenuBarPreview, menuBarOf, Seg, SignInStatus, StatusChip, Toggle, UpdatesSection } from "./components";
import MenuBarSettings from "./MenuBarSettings";
import * as f from "./format";
import { messageOf, nearestTextScale, parseDecimal, storedAllowance, TEXT_SCALES } from "./settingsModel";
import type { Settings, Snapshot, UpdateState } from "./types";

type Props = {
  snap: Snapshot;
  update: (p: Partial<Settings>) => Promise<void>;
  refresh: () => Promise<void>;
  busy: boolean;
  refreshFailed: boolean;
};

const SLIDER_KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);

function useDraft<T>(stored: T) {
  const [draft, setDraft] = useState(stored);
  useEffect(() => {
    setDraft(stored);
  }, [stored]);
  return [draft, setDraft] as const;
}

function Setting({ title, help, children }: { title: string; help?: ReactNode; children: ReactNode }) {
  return (
    <div className="setting">
      <div className="text"><div>{title}</div>{help && <div className="help">{help}</div>}</div>
      {children}
    </div>
  );
}

function Problem({ children }: { children: ReactNode }) {
  return <p className="help" role="alert" style={{ userSelect: "text" }}>{children}</p>;
}

const hhmm = (ts: number) => { const d = new Date(ts); return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`; };

function SettingsPreview({ snap }: { snap: Snapshot }) {
  const s = snap.settings;
  const clock = snap.now > 0 ? hhmm(snap.now) : null;
  return (
    <aside className="settings-preview" aria-label="Live preview">
      <div className="caps" style={{ marginBottom: "var(--space-2)" }}>Preview</div>
      <div className="sp-menubar">
        {clock && <span className="faint">{clock}</span>}
        <MenuBarPreview look={menuBarOf(s)} pace={snap.pace} alertColors={s.alertColors} />
      </div>
      <div className="sp-popover">
        <div className="row" style={{ marginBottom: "var(--space-2)", gap: 6 }}>
          <CopilotIcon size={15} /><b style={{ fontSize: 13 }}>Copilot</b><StatusChip snap={snap} />
        </div>
        <CycleSummaryView snap={snap} />
      </div>
    </aside>
  );
}

export default function SettingsPage({ snap, update, refresh, busy, refreshFailed }: Props) {
  const s = snap.settings;
  const n = s.notifications;
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [autostartError, setAutostartError] = useState<string | null>(null);
  const [threshold, setThreshold] = useState("");
  const addThreshold = () => {
    if (!threshold) return;
    update({ notifications: { ...n, thresholds: [...new Set([...n.thresholds, Number(threshold)])].sort((a, b) => a - b) } });
    setThreshold("");
  };
  const [allowance, setAllowance] = useDraft(storedAllowance(s.allowanceOverride));
  const [burst, setBurst] = useDraft(String(n.burstAic));
  const [tol, setTol] = useDraft(s.paceTolerancePct);
  const [hotkey, setHotkey] = useDraft(s.hotkey);
  const [cliDir, setCliDir] = useDraft(s.cliLogDir ?? "");
  const [resetting, setResetting] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);
  const [updateState, setUpdateState] = useState<UpdateState>({ state: "idle" });
  const [updateError, setUpdateError] = useState<string | null>(null);
  useEffect(() => { api.autostart().then(setAutostart).catch(() => setAutostart(null)); }, []);
  useEffect(() => {
    api.updateState().then(setUpdateState).catch(() => {});
    return api.onUpdate(setUpdateState);
  }, []);
  const ghEntitlement = snap.githubPlan?.entitlement;

  const toggleAutostart = (enable: boolean) => {
    setAutostartError(null);
    api.autostart(enable).then(setAutostart).catch((e) => setAutostartError(`Couldn't change launch at login: ${messageOf(e)}`));
  };
  const checkUpdates = () => {
    setUpdateError(null);
    api.checkForUpdates().then(setUpdateState).catch((e) => setUpdateError(`Couldn't check for updates: ${messageOf(e)}`));
  };
  const resetAll = async () => {
    setResetting(false);
    setResetError(null);
    try {
      const defaults = await api.defaultSettings();
      await update({ ...defaults, onboarded: true });
    } catch (e) {
      setResetError(`Couldn't reset settings: ${messageOf(e)}`);
    }
  };

  const allowanceValue = allowance.trim() === "" ? 0 : parseDecimal(allowance.trim());
  const burstValue = parseDecimal(burst);
  const commitAllowance = () => {
    if (allowanceValue === null) { setAllowance(storedAllowance(s.allowanceOverride)); return; }
    if (allowanceValue !== s.allowanceOverride) void update({ allowanceOverride: allowanceValue });
  };
  const commitBurst = () => {
    if (burstValue === null) { setBurst(String(n.burstAic)); return; }
    if (burstValue !== n.burstAic) void update({ notifications: { ...n, burstAic: burstValue } });
  };
  const commitTolerance = () => {
    if (tol !== s.paceTolerancePct) void update({ paceTolerancePct: tol });
  };
  const commitHotkey = () => {
    const next = hotkey.trim();
    if (next !== s.hotkey) void update({ hotkey: next });
  };
  const commitCliDir = () => {
    const next = cliDir.trim() || null;
    if (next !== s.cliLogDir) void update({ cliLogDir: next });
  };

  return (
    <div className="settings-layout">
      <SettingsPreview snap={snap} />
      <div className="settings-form">
        <div className="section-title">General</div>
        <div className="section">
          <Setting title="Launch at login">
            {inTauri ? (
              <>
                <Toggle label="Launch at login" checked={!!autostart} onChange={toggleAutostart} />
                {autostartError && <Problem>{autostartError}</Problem>}
              </>
            ) : <span className="faint" style={{ fontSize: 12 }}>In the app</span>}
          </Setting>
          <Setting title="Keyboard shortcut" help={<>e.g. <code>Alt+Cmd+C</code>. Leave empty to turn it off.</>}>
            <input className="field" style={{ width: 140 }} value={hotkey} onChange={(e) => setHotkey(e.target.value)} onBlur={commitHotkey}
              onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} aria-label="Keyboard shortcut" placeholder="Off" />
          </Setting>
          <Setting title="Theme">
            <Seg label="Theme" value={s.theme} options={[["system", "System"], ["light", "Light"], ["dark", "Dark"]]} onChange={(theme) => update({ theme })} />
          </Setting>
          <Setting title="Text size">
            <select className="field" value={String(nearestTextScale(s.textScale))} onChange={(e) => update({ textScale: Number(e.target.value) })} aria-label="Text size">
              {TEXT_SCALES.map((z) => <option key={z} value={String(z)}>{Math.round(z * 100)}%</option>)}
            </select>
          </Setting>
        </div>

        <div className="section-title">Menu bar</div>
        <div className="section" style={{ padding: "14px 14px 4px" }}>
          <MenuBarSettings snap={snap} update={update} />
        </div>

        <div className="section-title">Popover</div>
        <div className="section">
          <Setting title="Cycle summary">
            <Seg label="Cycle summary" value={s.cycleSummary} options={[["gauge", "Gauge"], ["bar", "Number + bar"]]} onChange={(cycleSummary) => update({ cycleSummary })} />
          </Setting>
          <Setting title="Headline number">
            <Seg label="Headline number" value={s.cycleHeadline} options={[["used", "AIC used"], ["remaining", "AIC left"]]} onChange={(cycleHeadline) => update({ cycleHeadline })} />
          </Setting>
        </div>

        <div className="section-title">Allowance</div>
        <div className="section">
          <Setting title="Allowance" help={snap.allowanceCapped && ghEntitlement ? `Capped at your Entitlement (${f.aic(ghEntitlement)} AIC): your plan doesn't permit Overage.` : ghEntitlement ? `Leave empty to use your Entitlement from GitHub (${f.aic(ghEntitlement)} AIC).` : "No Entitlement from GitHub. Set an Allowance to see your pace."}>
            <input className="field num" style={{ width: 96, textAlign: "right" }} inputMode="decimal" placeholder={ghEntitlement ? String(ghEntitlement) : "e.g. 300"} value={allowance}
              aria-invalid={allowanceValue === null} onChange={(e) => setAllowance(e.target.value.replace(/[^0-9.]/g, ""))}
              onBlur={commitAllowance} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} aria-label="Allowance in AIC" />
          </Setting>
        </div>

        <div className="section-title">Notifications</div>
        <div className="section">
          <Setting title="Send notifications">
            <Toggle label="Notifications" checked={n.enabled} onChange={(v) => update({ notifications: { ...n, enabled: v } })} />
          </Setting>
          <fieldset disabled={!n.enabled} style={{ border: 0, margin: 0, padding: 0, minWidth: 0, opacity: n.enabled ? 1 : 0.45 }}>
            <Setting title="When Used reaches" help="Percent of the Allowance.">
              <div className="row" style={{ flexWrap: "wrap", justifyContent: "flex-end", gap: 6 }}>
                {n.thresholds.map((t) => (
                  <span key={t} className="pill">{t}%<button aria-label={`Remove ${t}%`} onClick={() => update({ notifications: { ...n, thresholds: n.thresholds.filter((x) => x !== t) } })}>×</button></span>
                ))}
                <input className="field num" style={{ width: 56 }} placeholder="%" value={threshold} onChange={(e) => setThreshold(e.target.value.replace(/\D/g, ""))}
                  onKeyDown={(e) => e.key === "Enter" && addThreshold()} aria-label="New threshold in percent" />
                <button className="btn" disabled={!threshold} onClick={addThreshold}>Add</button>
              </div>
            </Setting>
            <Setting title="When I'm ahead of Even Pace">
              <Toggle label="Ahead of pace" checked={n.aheadOfPace} onChange={(v) => update({ notifications: { ...n, aheadOfPace: v } })} />
            </Setting>
            <Setting title="Burst alert" help="AIC within one hour. 0 = off.">
              <span className="row" style={{ gap: 6 }}>
                <input className="field num" style={{ width: 70, textAlign: "right" }} inputMode="decimal" value={burst} aria-invalid={burstValue === null}
                  onChange={(e) => setBurst(e.target.value.replace(/[^0-9.]/g, ""))} onBlur={commitBurst}
                  onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} aria-label="Burst alert threshold in AIC per hour" />
                <span className="faint" style={{ width: 52 }}>AIC / h</span>
              </span>
            </Setting>
          </fieldset>
        </div>

        <div className="section-title">Sign-in</div>
        <div className="section">
          <div className="setting">
            <div className="text"><SignInStatus signIn={snap.status.signIn} /></div>
            <button className="btn" disabled={busy} onClick={() => void refresh()}>Check again</button>
          </div>
          {refreshFailed && <div className="setting"><Problem>Couldn't check GitHub. Try again.</Problem></div>}
        </div>

        <div className="section-title">Updates</div>
        <UpdatesSection version={version} update={updateState} autoCheck={s.autoUpdateCheck}
          onCheck={checkUpdates} onAutoCheck={(v) => update({ autoUpdateCheck: v })} onOpen={() => api.openUpdate()} />
        {updateError && <Problem>{updateError}</Problem>}

        <div className="section-title">Logs</div>
        <div className="section">
          <Setting title="Copilot session logs" help={<code>~/.copilot/session-state</code>}>
            <span className="faint" style={{ fontSize: 12 }}>{s.sources.cli ? f.plural(snap.status.cliFiles, "session") : "Off"}</span>
          </Setting>
        </div>

        <details className="advanced">
          <summary className="section-title">Advanced</summary>
          <div className="section">
            <Setting title="“On pace” tolerance">
              <input type="range" min={0} max={20} step={1} value={tol} onChange={(e) => setTol(Number(e.target.value))}
                onPointerUp={commitTolerance} onKeyUp={(e) => { if (SLIDER_KEYS.has(e.key)) commitTolerance(); }} aria-label="Pace tolerance" />
              <span className="num" style={{ width: 32 }}>±{tol}%</span>
            </Setting>
            <Setting title="Week starts on Monday">
              <Toggle label="Week starts Monday" checked={s.weekStartsMonday} onChange={(v) => update({ weekStartsMonday: v })} />
            </Setting>
            <Setting title="Chart colours">
              <Seg label="Chart colours" value={s.palette} options={[["default", "Default"], ["colorblind", "Colour-blind safe"]]} onChange={(palette) => update({ palette })} />
            </Setting>
            <Setting title="Check GitHub every">
              <select className="field" value={s.refreshMinutes} onChange={(e) => update({ refreshMinutes: Number(e.target.value) })}>
                {[1, 2, 5, 10, 15, 30, 60].map((m) => <option key={m} value={m}>{f.plural(m, "minute")}</option>)}
              </select>
            </Setting>
            <Setting title="Read Copilot session logs"><Toggle label="Copilot session logs" checked={s.sources.cli} onChange={(v) => update({ sources: { ...s.sources, cli: v } })} /></Setting>
            <div style={{ display: "grid", gap: 8, padding: "10px 0" }}>
              <label className="row"><span style={{ width: 170 }} className="muted">Copilot CLI folder</span>
                <input className="field grow" placeholder="~/.copilot/session-state" value={cliDir} onChange={(e) => setCliDir(e.target.value)} onBlur={commitCliDir} /></label>
            </div>
            <Setting title="Reset all settings">
              {resetting ? (
                <span className="row" style={{ gap: 6 }}>
                  <span>Reset all settings?</span>
                  <button className="btn" onClick={() => void resetAll()}>Reset</button>
                  <button className="btn" onClick={() => setResetting(false)}>Cancel</button>
                </span>
              ) : (
                <button className="btn" onClick={() => setResetting(true)}>Reset…</button>
              )}
            </Setting>
            {resetError && <Problem>{resetError}</Problem>}
          </div>
        </details>

        <div className="row" style={{ marginTop: 18 }}>
          <p className="faint grow" style={{ fontSize: 11.5, margin: 0 }}>CopilotBar v{version}</p>
          <button className="btn" onClick={() => api.quit()}>Quit CopilotBar</button>
        </div>
      </div>
    </div>
  );
}
