import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

const ROOT = ":root {";
const DARK_MEDIA = ':root:not([data-theme="light"]) {';
const DARK_ATTR = ':root[data-theme="dark"] {';
const COLOURBLIND = ':root[data-palette="colorblind"] {';
const COLOURBLIND_DARK_MEDIA = ':root[data-palette="colorblind"]:not([data-theme="light"]) {';
const COLOURBLIND_DARK_ATTR = ':root[data-palette="colorblind"][data-theme="dark"] {';

function block(selector: string): Record<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) throw new Error(`selector not found: ${selector}`);
  const body = css.slice(start + selector.length, css.indexOf("}", start));
  const tokens: Record<string, string> = {};
  for (const m of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) tokens[m[1]] = m[2].trim();
  return tokens;
}

type RGBA = [number, number, number, number];

function parseColour(value: string): RGBA {
  const hex = /^#([0-9a-f]{6})$/i.exec(value);
  if (hex) {
    const n = parseInt(hex[1], 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255, 1];
  }
  const rgba = /^rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)$/.exec(value);
  if (rgba) return [Number(rgba[1]), Number(rgba[2]), Number(rgba[3]), Number(rgba[4])];
  throw new Error(`unsupported colour: ${value}`);
}

const channel = (c: number) => {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
const luminance = ([r, g, b]: RGBA) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);

function contrast(a: RGBA, b: RGBA): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function over(fg: RGBA, bg: RGBA): RGBA {
  const [r, g, b, a] = fg;
  return [r * a + bg[0] * (1 - a), g * a + bg[1] * (1 - a), b * a + bg[2] * (1 - a), 1];
}

const lightBase = block(ROOT);
const darkBase = block(DARK_MEDIA);
const colourblindLight = block(COLOURBLIND);
const colourblindDark = block(COLOURBLIND_DARK_MEDIA);

const modes: Record<string, Record<string, string>> = {
  light: lightBase,
  dark: darkBase,
  "colour-blind light": { ...lightBase, ...colourblindLight },
  "colour-blind dark": { ...darkBase, ...colourblindLight, ...colourblindDark },
};

const TEXT_TOKENS = ["--orange-text", "--green-text", "--red-text", "--link"];
const WHITE: RGBA = [255, 255, 255, 1];

describe("text colour contrast", () => {
  it.each(Object.entries(modes))("%s: text tokens reach 4.5:1 on the background and on both card tints", (_, tokens) => {
    const bg = parseColour(tokens["--bg"]);
    const surfaces = { background: bg, card: over(parseColour(tokens["--card"]), bg), cardStrong: over(parseColour(tokens["--card-strong"]), bg) };
    for (const name of TEXT_TOKENS) {
      const text = parseColour(tokens[name]);
      for (const [surface, colour] of Object.entries(surfaces)) {
        expect(contrast(text, colour), `${name} on ${surface}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it.each(Object.entries(modes))("%s: white on the primary fill reaches 4.5:1", (_, tokens) => {
    expect(contrast(WHITE, parseColour(tokens["--accent-fill"]))).toBeGreaterThanOrEqual(4.5);
  });
});

describe("dark palette blocks", () => {
  it("keeps the media-query and data-theme dark blocks identical", () => {
    expect(block(DARK_ATTR)).toEqual(darkBase);
  });

  it("keeps the colour-blind dark overrides identical", () => {
    expect(block(COLOURBLIND_DARK_ATTR)).toEqual(colourblindDark);
  });
});

describe("colour use in styles", () => {
  it("uses the text tokens for coloured text", () => {
    expect(css).toMatch(/\.link \{[^}]*color: var\(--link\)/);
    expect(css).toMatch(/\.footer-ok \{ color: var\(--green-text\)/);
  });

  it("keeps the fill tokens out of text colour", () => {
    expect(css).not.toMatch(/(?<![-\w])color:\s*var\(--(orange|green|red|blue)\)/);
  });

  it("fills primary buttons and the active tab with the primary fill token", () => {
    expect(css).toMatch(/\.btn\.primary \{ background: var\(--accent-fill\)/);
    expect(css).toMatch(/\.tab\.on \{ background: var\(--accent-fill\)/);
  });
});

describe("focus and selection", () => {
  it("draws a focus ring on the pill remove button", () => {
    expect(css).toMatch(/\.pill button:focus-visible \{ outline: 2px solid var\(--accent\)/);
  });

  it("lets error and help text be selected", () => {
    expect(css).toMatch(/\.banner, \.help \{ user-select: text; -webkit-user-select: text; \}/);
  });
});
