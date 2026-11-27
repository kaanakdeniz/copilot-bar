import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { UpdatesSection } from "./components";
import type { UpdateState } from "./types";

const noop = () => {};
const html = (update: UpdateState, autoCheck = true) =>
  renderToStaticMarkup(createElement(UpdatesSection, { version: "1.2.0", update, autoCheck, onCheck: noop, onAutoCheck: noop, onOpen: noop }));
const text = (update: UpdateState, autoCheck = true) => html(update, autoCheck).replace(/<[^>]+>/g, " ").replace(/&#x27;|&#39;/g, "'").replace(/\s+/g, " ");
const buttons = (update: UpdateState) =>
  [...html(update).matchAll(/<button([^>]*)>(.*?)<\/button>/g)].map(([, attrs, label]) => (attrs.includes("disabled") ? `${label} (disabled)` : label));

describe("Settings' Updates section", () => {
  it("shows the running version and the automatic-check switch in every state", () => {
    for (const u of [{ state: "idle" }, { state: "checking" }, { state: "upToDate" }, { state: "available", version: "1.3.0", url: "u" }, { state: "downloading", version: "1.3.0" }, { state: "installing", version: "1.3.0" }, { state: "error", message: "x", url: null }] as UpdateState[]) {
      expect(text(u)).toContain("CopilotBar 1.2.0");
      expect(text(u)).toContain("Automatically check for updates");
    }
  });
  it("the switch shows the setting and when automatic checks run", () => {
    expect(html({ state: "idle" }, true)).toMatch(/type="checkbox" checked=""/);
    expect(html({ state: "idle" }, false)).not.toMatch(/checked=""/);
    expect(text({ state: "idle" })).toContain("At launch and once a day.");
  });
  it("not checked yet", () => {
    expect(text({ state: "idle" })).toContain("Not checked yet");
    expect(buttons({ state: "idle" })).toEqual(["Check for Updates"]);
  });
  it("checking: says so and can't be started twice", () => {
    expect(text({ state: "checking" })).toContain("Checking for updates…");
    expect(buttons({ state: "checking" })).toEqual(["Check for Updates (disabled)"]);
  });
  it("up to date", () => {
    expect(text({ state: "upToDate" })).toContain("You're up to date");
    expect(buttons({ state: "upToDate" })).toEqual(["Check for Updates"]);
  });
  it("available: names the version and offers the update", () => {
    const u: UpdateState = { state: "available", version: "1.3.0", url: "https://github.com/o/n/releases/tag/v1.3.0" };
    expect(text(u)).toContain("CopilotBar 1.3.0 is available");
    expect(buttons(u)).toEqual(["Update to 1.3.0", "Check for Updates"]);
  });
  it("downloading: names the version and can't be started twice", () => {
    const u: UpdateState = { state: "downloading", version: "1.3.0" };
    expect(text(u)).toContain("Downloading CopilotBar 1.3.0…");
    expect(buttons(u)).toEqual(["Check for Updates (disabled)"]);
  });
  it("installing: names the version and can't be started twice", () => {
    const u: UpdateState = { state: "installing", version: "1.3.0" };
    expect(text(u)).toContain("Installing CopilotBar 1.3.0…");
    expect(buttons(u)).toEqual(["Check for Updates (disabled)"]);
  });
  it("a failed install says why and links to the release page", () => {
    const u: UpdateState = { state: "error", message: "The update's signature was made with a different key than CopilotBar's update key, so nothing was installed.", url: "https://github.com/o/n/releases/latest" };
    expect(text(u)).toContain("nothing was installed");
    expect(buttons(u)).toEqual(["Open the release page", "Check for Updates"]);
  });
  it("error: says what went wrong and links to the release page", () => {
    const u: UpdateState = { state: "error", message: "Can't reach GitHub: check your internet connection", url: "https://github.com/o/n/releases/latest" };
    expect(text(u)).toContain("Can't reach GitHub");
    expect(buttons(u)).toEqual(["Open the release page", "Check for Updates"]);
  });
  it("error without a known release page has no link", () => {
    expect(buttons({ state: "error", message: "No repository", url: null })).toEqual(["Check for Updates"]);
  });
});
