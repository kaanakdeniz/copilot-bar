import { MENU_BAR_ICONS, MENU_BAR_NUMBERS, MenuBarIconMark, menuBarOf, onRadioKeyDown, Seg, Toggle } from "./components";
import type { MenuBarDisplay, MenuBarLook, Settings, Snapshot } from "./types";

const DISPLAYS: [MenuBarDisplay, string][] = [["icon-number", "Icon + number"], ["icon", "Icon only"], ["number", "Number only"]];

export default function MenuBarSettings({ snap, update, compact }: { snap: Snapshot; update: (p: Partial<Settings>) => void; compact?: boolean }) {
  const s = snap.settings;
  const look = menuBarOf(s);
  const note = look.display === "number" ? "Needs an icon." : null;
  const iconIndex = Math.max(0, MENU_BAR_ICONS.findIndex((o) => o.key === look.icon));
  const set = (p: Partial<MenuBarLook>) => {
    const n = { ...look, ...p };
    update({ menuBarDisplay: n.display, menuBarIcon: n.icon, menuBarNumber: n.number });
  };
  return (
    <div className={compact ? "mb-set compact" : "mb-set"}>
      <div className="mb-label">Display</div>
      <Seg label="Display" value={look.display} options={DISPLAYS} onChange={(display) => set({ display })} className="mb-seg" radio />

      {look.display !== "number" && (
        <>
          <div className="mb-label">Icon</div>
          <div className="mb-icons" role="radiogroup" aria-label="Icon"
            onKeyDown={(e) => onRadioKeyDown(e, iconIndex, MENU_BAR_ICONS.length, (i) => set({ icon: MENU_BAR_ICONS[i].key }))}>
            {MENU_BAR_ICONS.map((o, i) => (
              <button key={o.key} role="radio" aria-checked={look.icon === o.key} tabIndex={i === iconIndex ? 0 : -1} className={look.icon === o.key ? "on" : ""} onClick={() => set({ icon: o.key })}>
                <MenuBarIconMark icon={o.key} pace={snap.pace} size={20} />
                <span>{o.title}</span>
              </button>
            ))}
          </div>
        </>
      )}

      {look.display !== "icon" && (
        <label className="mb-row">
          <span className="grow">Number</span>
          <select className="field" value={look.number} onChange={(e) => set({ number: e.target.value as MenuBarLook["number"] })}>
            {MENU_BAR_NUMBERS.map((o) => <option key={o.key} value={o.key}>{o.title}</option>)}
          </select>
        </label>
      )}

      <div className="mb-row" style={{ alignItems: note ? "flex-start" : "center" }}>
        <div className="grow">
          <div>Alert colours</div>
          {note && <div className="help">{note}</div>}
        </div>
        <Toggle label="Alert colours" checked={s.alertColors} onChange={(v) => update({ alertColors: v })} />
      </div>
    </div>
  );
}
