import { useState } from "react";
import { MenuBarPreview, menuBarOf } from "../components";
import Popover, { type PopoverStart } from "../Popover";
import { useSnapshot } from "../state";

const params = new URLSearchParams(location.search);
const START: Partial<Record<string, PopoverStart>> = { overview: "overview", expanded: "expanded", advanced: "expanded", calendar: "calendar" };

export default function Preview() {
  const { snap } = useSnapshot();
  const [open, setOpen] = useState(true);
  return (
    <div style={{
      minHeight: "100vh", position: "relative",
      background: "radial-gradient(1200px 700px at 20% 10%, #b6c8ff 0%, transparent 60%), radial-gradient(900px 600px at 90% 90%, #ffc6e3 0%, transparent 55%), #e9ecf5",
    }} className="preview-desktop">
      <style>{`@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) .preview-desktop { background: radial-gradient(1200px 700px at 20% 10%, #2b3a78 0%, transparent 60%), radial-gradient(900px 600px at 90% 90%, #5a2350 0%, transparent 55%), #121318 !important; } }
        :root[data-theme="dark"] .preview-desktop { background: radial-gradient(1200px 700px at 20% 10%, #2b3a78 0%, transparent 60%), radial-gradient(900px 600px at 90% 90%, #5a2350 0%, transparent 55%), #121318 !important; }
        .fake-bar .menubar-preview { background: transparent; border: none; min-width: 0; }`}</style>
      <div className="fake-bar row" style={{ height: 28, padding: "0 12px", gap: 16, background: "color-mix(in srgb, var(--panel) 60%, transparent)", backdropFilter: "blur(30px)", fontSize: 13 }}>
        <b>Finder</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span>Help</span>
        <span className="grow" />
        <button onClick={() => setOpen(!open)} style={{ all: "unset", cursor: "pointer", borderRadius: 5, background: open ? "rgba(127,127,127,.25)" : undefined, padding: "0 2px" }}>
          {snap && <MenuBarPreview look={menuBarOf(snap.settings)} pace={snap.pace} alertColors={snap.settings.alertColors} />}
        </button>
        <span style={{ whiteSpace: "nowrap" }}>{snap ? new Date(snap.now).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit", hour12: false }) : ""}</span>
      </div>
      {open && <div style={{ position: "absolute", top: 34, right: 90 }}><Popover start={START[params.get("mode") ?? ""]} remember={!params.has("studio")} /></div>}
    </div>
  );
}
