import { Component, StrictMode, type ComponentType, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { inTauri } from "./backend";
import Dashboard from "./Dashboard";
import Popover from "./Popover";
import "./styles.css";

const params = new URLSearchParams(location.search);
const view = params.get("view") ?? "popover";
const root = document.documentElement;
if (view === "popover") root.classList.add("popover");
if (!inTauri) root.classList.add("preview");

class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div className="empty" role="alert">
        <p>Something went wrong</p>
        <button className="btn" onClick={() => location.reload()}>Reload</button>
      </div>
    );
  }
}

async function appFor(): Promise<ComponentType> {
  if (view === "dashboard") return Dashboard;
  if (inTauri) return Popover;
  if (!import.meta.env.DEV) return Popover;
  if (view === "studio") return (await import("./dev/Studio")).default;
  if (view === "icons") return (await import("./dev/IconLab")).default;
  return (await import("./dev/Preview")).default;
}

async function boot() {
  if (!inTauri && import.meta.env.DEV) await import("./dev/browser");
  const App = await appFor();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </StrictMode>,
  );
}

void boot();
