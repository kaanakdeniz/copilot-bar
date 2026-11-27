import { useState } from "react";
import { MENU_BAR_ICONS, MENU_BAR_NUMBERS, MenuBarPreview } from "../components";
import { useSnapshot } from "../state";
import type { MenuBarLook, Pace, PaceStatus } from "../types";

const ROWS: { key: string; title: string; look: MenuBarLook }[] = [
  ...MENU_BAR_ICONS.map((i) => ({ key: i.key, title: `${i.title} + percent`, look: { display: "icon-number", icon: i.key, number: "pct" } as MenuBarLook })),
  ...MENU_BAR_NUMBERS.slice(1).map((n) => ({ key: n.key, title: `Disc + ${n.title.toLowerCase()}`, look: { display: "icon-number", icon: "disc", number: n.key } as MenuBarLook })),
];

const WALLPAPERS: { name: string; bg: string; fg: string }[] = [
  { name: "Light", bg: "#e9e7ec", fg: "#1d1d1f" },
  { name: "Dark", bg: "#2a2830", fg: "#fff" },
  { name: "Busy photo", bg: "linear-gradient(90deg,#c9a46a,#5e7f9a 40%,#e8e0d0 70%,#33402f)", fg: "#fff" },
];

const STATES: { name: string; frac: number | null; status: PaceStatus }[] = [
  { name: "No Allowance", frac: null, status: "noAllowance" },
  { name: "42% · on pace", frac: 0.42, status: "onPace" },
  { name: "71% · ahead", frac: 0.71, status: "ahead" },
  { name: "100% · Exhausted", frac: 1, status: "exhausted" },
];

export default function IconLab() {
  const { snap } = useSnapshot();
  const [scale, setScale] = useState(2);
  const [wp, setWp] = useState(0);
  const walls = [WALLPAPERS[wp]];
  if (!snap) return null;
  const paceFor = (s: (typeof STATES)[number]): Pace => ({
    ...snap.pace,
    status: s.status,
    fracUsed: s.frac,
    allowance: s.frac == null ? null : 300,
    used: s.frac == null ? 1.73 : s.frac * 300,
  });
  return (
    <div style={{ padding: 20, display: "grid", gap: 18 }}>
      <style>{`.lab-bar .menubar-preview { background: transparent; border: none; min-width: 0; padding: 0; color: inherit; }
        .lab-bar .menubar-preview .num { color: inherit !important; }`}</style>
      <div className="row" style={{ gap: 12 }}>
        <b style={{ fontSize: 16 }}>Menu bar icon lab</b>
        <span className="muted">Magnify</span>
        {[1, 2, 3].map((z) => (
          <button key={z} className={z === scale ? "btn primary" : "btn"} onClick={() => setScale(z)}>{z}×</button>
        ))}
        <span className="muted" style={{ marginLeft: 12 }}>Wallpaper</span>
        {WALLPAPERS.map((w, i) => (
          <button key={w.name} className={i === wp ? "btn primary" : "btn"} onClick={() => setWp(i)}>{w.name}</button>
        ))}
      </div>
      <table style={{ borderCollapse: "separate", borderSpacing: "8px 6px" }}>
        <thead>
          <tr>
            <th />
            {walls.map((w) => <th key={w.name} colSpan={STATES.length} style={{ textAlign: "left", fontWeight: 600 }}>{w.name}</th>)}
          </tr>
          <tr>
            <th />
            {walls.flatMap((w) => STATES.map((s) => <th key={w.name + s.name} className="faint" style={{ fontWeight: 400, fontSize: 11, textAlign: "left" }}>{s.name}</th>))}
          </tr>
        </thead>
        <tbody>
          {ROWS.map((o) => (
            <tr key={o.key}>
              <td style={{ whiteSpace: "nowrap", paddingRight: 8 }}>{o.title}<div className="faint" style={{ fontSize: 11 }}>{o.key}</div></td>
              {walls.flatMap((w) => STATES.map((s) => (
                <td key={w.name + s.name}>
                  <div className="lab-bar" style={{ background: w.bg, color: w.fg, height: 24, padding: "0 8px", display: "inline-flex", alignItems: "center", borderRadius: 4, zoom: scale }}>
                    <MenuBarPreview look={o.look} pace={paceFor(s)} alertColors={snap.settings.alertColors} />
                  </div>
                </td>
              )))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
