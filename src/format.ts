import type { Metric } from "./analytics";

const SUB_CENT = 0.01;
const THOUSAND = 1_000;
const CELL_THOUSANDS_DIGITS = 1;

const formatters = new Map<string, Intl.NumberFormat>();
function formatter(min: number, max: number): Intl.NumberFormat {
  const key = `${min}:${max}`;
  let f = formatters.get(key);
  if (!f) {
    f = new Intl.NumberFormat(undefined, { minimumFractionDigits: min, maximumFractionDigits: max });
    formatters.set(key, f);
  }
  return f;
}
const fixed = (v: number, digits: number) => formatter(digits, digits).format(v);
const trimmed = (v: number, digits: number) => formatter(0, digits).format(v);
const roundTo = (v: number, digits: number) => { const scale = 10 ** digits; return Math.round(v * scale) / scale; };

function settle(v: number, digitsFor: (x: number) => number): { value: number; digits: number } {
  const digits = digitsFor(roundTo(v, digitsFor(v)));
  return { value: roundTo(v, digits), digits };
}

const aicDigits = (x: number) => (x < 10 ? 2 : x < 100 ? 1 : 0);
const cellDigits = (x: number) => (x < 1 ? 2 : x < 10 ? 1 : 0);
const shortDigits = (x: number) => (x < 10 ? 1 : 0);
const kDigits = (x: number) => (x < 10 ? 1 : 0);
const unitDigits = (x: number) => (x < 100 ? 1 : 0);

export function aic(v: number): string {
  const a = Math.abs(v);
  if (a === 0) return "0";
  if (a < SUB_CENT) return "<0.01";
  const { value, digits } = settle(a, aicDigits);
  return (v < 0 ? "-" : "") + fixed(value, digits);
}

export function cellAic(v: number): string {
  const plain = settle(v, cellDigits);
  if (plain.value < THOUSAND) return fixed(plain.value, plain.digits);
  const k = settle(v / THOUSAND, kDigits);
  return `${trimmed(k.value, CELL_THOUSANDS_DIGITS)}k`;
}

export const short = (v: number) => {
  const { value, digits } = settle(v, shortDigits);
  return fixed(value, digits);
};

const TOKEN_UNITS = [{ scale: 1e3, suffix: "k" }, { scale: 1e6, suffix: "M" }, { scale: 1e9, suffix: "B" }];

export function tokens(v: number): string {
  const a = Math.abs(v);
  const sign = v < 0 ? "-" : "";
  const whole = Math.round(a);
  if (whole < THOUSAND) return (whole === 0 ? "" : sign) + trimmed(whole, 0);
  const scaled = TOKEN_UNITS.map(({ scale, suffix }) => ({ suffix, ...settle(a / scale, unitDigits) }));
  const unit = scaled.find((u) => u.value < THOUSAND) ?? scaled[scaled.length - 1];
  return sign + trimmed(unit.value, unit.digits) + unit.suffix;
}

export const count = (v: number) => fixed(Math.round(v), 0);

export const metricValue = (v: number, m: Metric) => (m === "aic" ? aic(v) : m === "requests" ? count(v) : tokens(v));

export function axisValue(v: number, step: number, m: Metric): string {
  if (m !== "aic" && m !== "requests") return tokens(v);
  if (step >= 1) return count(v);
  const digits = Math.min(2, Math.max(1, Math.ceil(-Math.log10(step))));
  return fixed(v, digits);
}
export const pct = (v: number) => `${Math.round(v * 100)}%`;
export const plural = (n: number, w: string) => `${count(n)} ${w}${n === 1 ? "" : "s"}`;

export const resetDay = (ts: number) => new Date(ts).toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "UTC" });

export function niceScale(max: number, ticks = 3): { max: number; step: number } {
  if (!(max > 0)) return { max: 1, step: 0.5 };
  const raw = max / ticks;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  return { max: Math.ceil(max / step) * step, step };
}

export const monthDay = (ts: number) => new Date(ts).toLocaleDateString(undefined, { month: "short", day: "numeric" });
export const weekdayDate = (ts: number) => new Date(ts).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
export const longDate = (ts: number) => new Date(ts).toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
export const monthName = (ts: number) => new Date(ts).toLocaleDateString(undefined, { month: "long" });

const MINUTE_S = 60;
const HOUR_S = 3_600;
const DAY_H = 24;

export function ago(ts: number | null, now = Date.now()): string {
  if (!ts) return "never";
  const s = Math.round((now - ts) / 1000);
  if (s < MINUTE_S) return "just now";
  const minutes = Math.round(s / MINUTE_S);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(s / HOUR_S);
  if (hours < DAY_H) return `${hours} h ago`;
  return monthDay(ts);
}
