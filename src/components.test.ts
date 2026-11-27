import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { bigNumberSize, menuBarAlertColour, menuBarOf, MenuBarPreview, PaceBar, radioStep, Seg, STATUS, statusMessage } from "./components";
import { buildDemo, DEFAULT_SETTINGS } from "./dev/demo";
import type { MenuBarLook, Pace, PaceStatus, Snapshot } from "./types";

describe("menu bar look", () => {
  it("reads the three stored fields", () => {
    expect(menuBarOf({ menuBarDisplay: "icon", menuBarIcon: "ring", menuBarNumber: "left" })).toEqual({ display: "icon", icon: "ring", number: "left" });
  });
  it("falls back to the shipped look for empty fields", () => {
    expect(menuBarOf({ menuBarDisplay: "", menuBarIcon: "", menuBarNumber: "" })).toEqual({ display: "icon-number", icon: "disc", number: "pct" });
  });
});

const text = (snap: Snapshot) => renderToStaticMarkup(createElement(Fragment, null, statusMessage(snap))).replace(/<[^>]+>/g, "");
const studio = (used: number, allowanceOverride: number) =>
  buildDemo({ used, allowance: 300, day: 16, today: 0, localShare: 1, overagePermitted: false, connection: "ok" }, { ...DEFAULT_SETTINGS, allowanceOverride });

describe("Exhausted message", () => {
  it("own Allowance exhausted on a metered plan: Copilot keeps working until the Entitlement runs out", () => {
    expect(text(studio(150, 120))).toContain("keeps working until your Entitlement");
  });
  it("own Allowance exhausted on an unlimited plan: no Entitlement to run out", () => {
    const s = studio(150, 120);
    const msg = text({ ...s, githubPlan: { ...s.githubPlan!, unlimited: true } });
    expect(msg).toContain("Your Allowance is exhausted");
    expect(msg).not.toContain("Entitlement");
  });
});

const STATUSES: PaceStatus[] = ["noAllowance", "under", "onPace", "ahead", "exhausted"];

describe("menu bar alert tint", () => {
  it.each([
    ["noAllowance", undefined],
    ["under", "var(--green)"],
    ["onPace", "var(--green)"],
    ["ahead", "var(--orange)"],
    ["exhausted", "var(--red)"],
  ] as const)("%s tints like the tray", (status, colour) => {
    expect(menuBarAlertColour(status, true)).toBe(colour);
  });

  it("is off for every status when alert colours are disabled", () => {
    for (const status of STATUSES) expect(menuBarAlertColour(status, false), status).toBeUndefined();
  });

  it("tints the preview icon with the same colour", () => {
    const look: MenuBarLook = { display: "icon", icon: "copilot", number: "pct" };
    const pace = { status: "under", fracUsed: 0.5, fracElapsed: 0.4, used: 150, allowance: 300 } as Pace;
    expect(renderToStaticMarkup(createElement(MenuBarPreview, { look, pace, alertColors: true }))).toContain('fill="var(--green)"');
  });
});

describe("status text colours", () => {
  it("uses text tokens for chips and fill tokens for bars", () => {
    expect(STATUS.under).toMatchObject({ color: "var(--blue)", text: "var(--link)" });
    expect(STATUS.onPace).toMatchObject({ color: "var(--green)", text: "var(--green-text)" });
    expect(STATUS.ahead).toMatchObject({ color: "var(--orange)", text: "var(--orange-text)" });
    expect(STATUS.exhausted).toMatchObject({ color: "var(--red)", text: "var(--red-text)" });
    expect(STATUS.noAllowance).toMatchObject({ color: "var(--muted)", text: "var(--muted)" });
  });
});

describe("gauge number size", () => {
  it("fits the headline into the budget", () => {
    expect(bigNumberSize("1,234.56", "/ 300")).toBeCloseTo(25, 10);
  });
  it("caps at the maximum for short numbers", () => {
    expect(bigNumberSize("12")).toBe(36);
  });
  it("floors at the minimum for long numbers", () => {
    expect(bigNumberSize("1234567890", "/ 1,000.00")).toBe(20);
  });
});

describe("PaceBar accessibility values", () => {
  const bar = (fracUsed: number) => renderToStaticMarkup(createElement(PaceBar, { pace: { status: "exhausted", fracUsed, fracElapsed: 0.5 } as Pace }));
  it("clamps aria-valuenow to the maximum", () => {
    expect(bar(1.4)).toContain('aria-valuenow="100"');
  });
  it("clamps aria-valuenow to the minimum", () => {
    expect(bar(-0.2)).toContain('aria-valuenow="0"');
  });
  it("reports the share used in whole percent", () => {
    expect(bar(0.426)).toContain('aria-valuenow="43"');
  });
});

describe("radio keyboard steps", () => {
  it.each([
    ["ArrowRight", 0, 4, 1],
    ["ArrowDown", 1, 4, 2],
    ["ArrowRight", 3, 4, 0],
    ["ArrowLeft", 2, 4, 1],
    ["ArrowUp", 0, 4, 3],
    ["Enter", 1, 4, null],
  ] as const)("%s from %i of %i moves to %s", (key, index, count, next) => {
    expect(radioStep(key, index, count)).toBe(next);
  });
});

describe("Seg radio semantics", () => {
  const options: readonly (readonly [string, string])[] = [["icon-number", "Icon + number"], ["icon", "Icon only"]];
  const render = (radio: boolean) => renderToStaticMarkup(createElement(Seg, { label: "Display", value: "icon", options, onChange: () => {}, radio }));

  it("renders a radiogroup with checked radios and roving focus", () => {
    const html = render(true);
    expect(html).toContain('role="radiogroup"');
    expect(html).toContain('<button role="radio" aria-checked="false" tabindex="-1" class="">Icon + number</button>');
    expect(html).toContain('<button role="radio" aria-checked="true" tabindex="0" class="on">Icon only</button>');
  });

  it("keeps the pressed-button group for other uses", () => {
    const html = render(false);
    expect(html).toContain('role="group"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).not.toContain("radio");
  });
});
