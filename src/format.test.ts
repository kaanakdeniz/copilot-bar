import { describe, expect, it } from "vitest";
import { ago, aic, axisValue, cellAic, count, monthDay, short, tokens } from "./format";

const num = (v: number, digits: number) => new Intl.NumberFormat(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(v);

describe("AIC formats", () => {
  it("groups thousands for totals", () => {
    expect(aic(176)).toBe("176");
    expect(aic(10500)).toBe((10500).toLocaleString());
  });
  it("shows two decimals under 10, one under 100 and none above", () => {
    expect(aic(1.234)).toBe(num(1.23, 2));
    expect(aic(42.31)).toBe(num(42.3, 1));
    expect(aic(206.4)).toBe("206");
  });
  it("rounds before it picks the bracket", () => {
    expect(aic(9.996)).toBe(num(10, 1));
    expect(aic(9.994)).toBe(num(9.99, 2));
    expect(aic(99.96)).toBe("100");
    expect(aic(999.7)).toBe((1000).toLocaleString());
  });
  it("keeps zero and sub-cent values readable", () => {
    expect(aic(0)).toBe("0");
    expect(aic(0.004)).toBe("<0.01");
    expect(aic(0.01)).toBe(num(0.01, 2));
  });
  it("keeps the sign on negative values", () => {
    expect(aic(-5.5)).toBe(`-${num(5.5, 2)}`);
  });
  it("keeps calendar cells short", () => {
    expect(cellAic(8.34)).toBe(num(8.3, 1));
    expect(cellAic(12.4)).toBe("12");
    expect(cellAic(640)).toBe("640");
    expect(cellAic(1000)).toBe("1k");
    expect(cellAic(1240)).toBe(`${num(1.2, 1)}k`);
    expect(cellAic(12400)).toBe("12k");
  });
  it("rounds calendar cells before it switches to thousands", () => {
    expect(cellAic(999.6)).toBe("1k");
    expect(cellAic(9.96)).toBe("10");
    expect(cellAic(0.996)).toBe(num(1, 1));
  });
  it("keeps small calendar values distinguishable", () => {
    expect(cellAic(0.12)).toBe(num(0.12, 2));
  });
  it("shows the short form used in legends", () => {
    expect(short(3.3)).toBe(num(3.3, 1));
    expect(short(1234)).toBe((1234).toLocaleString());
    expect(short(999.6)).toBe((1000).toLocaleString());
  });
  it("counts whole numbers", () => {
    expect(count(1234.4)).toBe((1234).toLocaleString());
  });
});

describe("token formats", () => {
  it("shortens tokens without trailing .0", () => {
    expect(tokens(295000)).toBe("295k");
    expect(tokens(19100)).toBe("19.1k");
    expect(tokens(1_000_000)).toBe("1M");
    expect(tokens(600_000_000)).toBe("600M");
  });
  it("moves to the next unit instead of printing 1000k or 1000M", () => {
    expect(tokens(999_400)).toBe("999k");
    expect(tokens(999_960)).toBe("1M");
    expect(tokens(999_999)).toBe("1M");
    expect(tokens(999_999_999)).toBe("1B");
  });
  it("keeps small counts whole", () => {
    expect(tokens(0)).toBe("0");
    expect(tokens(12)).toBe("12");
    expect(tokens(999)).toBe("999");
    expect(tokens(1_234_567)).toBe(`${num(1.2, 1)}M`);
  });
});

describe("axis labels", () => {
  it("uses one precision per axis", () => {
    expect([0, 5, 10, 15].map((v) => axisValue(v, 5, "aic"))).toEqual(["0", "5", "10", "15"]);
    expect([0, 0.5, 1].map((v) => axisValue(v, 0.5, "aic"))).toEqual([num(0, 1), num(0.5, 1), num(1, 1)]);
    expect(axisValue(20000, 10000, "aic")).toBe((20000).toLocaleString());
  });
});

describe("ago", () => {
  const now = Date.UTC(2026, 10, 20, 12);
  it("says just now for the first minute", () => {
    expect(ago(now - 59_000, now)).toBe("just now");
    expect(ago(null, now)).toBe("never");
  });
  it("counts minutes up to the hour", () => {
    expect(ago(now - 60_000, now)).toBe("1 min ago");
    expect(ago(now - 3_569_000, now)).toBe("59 min ago");
  });
  it("never says 60 min ago; it moves to hours at the boundary", () => {
    expect(ago(now - 3_570_000, now)).toBe("1 h ago");
    expect(ago(now - 3_600_000, now)).toBe("1 h ago");
  });
  it("counts hours up to a day, then shows the date", () => {
    expect(ago(now - 23 * 3_600_000, now)).toBe("23 h ago");
    expect(ago(now - 86_340_000, now)).toBe(monthDay(now - 86_340_000));
  });
});
