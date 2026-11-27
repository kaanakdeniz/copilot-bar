import { describe, expect, it } from "vitest";
import { nearestTextScale, parseDecimal, storedAllowance } from "./settingsModel";

describe("parseDecimal", () => {
  it("accepts whole numbers and up to two decimals", () => {
    expect(parseDecimal("300")).toBe(300);
    expect(parseDecimal("12.5")).toBe(12.5);
    expect(parseDecimal("12.25")).toBe(12.25);
    expect(parseDecimal("7.")).toBe(7);
  });
  it("rejects text that is not a plain decimal", () => {
    expect(parseDecimal("")).toBeNull();
    expect(parseDecimal(".")).toBeNull();
    expect(parseDecimal(".5")).toBeNull();
    expect(parseDecimal("1.2.3")).toBeNull();
    expect(parseDecimal("12.255")).toBeNull();
    expect(parseDecimal("-1")).toBeNull();
    expect(parseDecimal("1e3")).toBeNull();
  });
});

describe("nearestTextScale", () => {
  it("keeps an offered scale and snaps anything else to the closest one", () => {
    expect(nearestTextScale(1)).toBe(1);
    expect(nearestTextScale(1.15)).toBe(1.15);
    expect(nearestTextScale(1.1)).toBe(1.15);
    expect(nearestTextScale(1.2)).toBe(1.15);
    expect(nearestTextScale(1.5)).toBe(1.3);
    expect(nearestTextScale(0.9)).toBe(1);
  });
});

describe("storedAllowance", () => {
  it("shows an empty field when no override is set", () => {
    expect(storedAllowance(0)).toBe("");
    expect(storedAllowance(300)).toBe("300");
    expect(storedAllowance(12.5)).toBe("12.5");
  });
});
