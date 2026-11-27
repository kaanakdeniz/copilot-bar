const DECIMAL_TEXT = /^\d+(\.\d{0,2})?$/;

export const TEXT_SCALES = [1, 1.15, 1.3] as const;

export function parseDecimal(text: string): number | null {
  return DECIMAL_TEXT.test(text) ? Number(text) : null;
}

export function nearestTextScale(scale: number): number {
  return TEXT_SCALES.reduce((best, z) => (Math.abs(z - scale) < Math.abs(best - scale) ? z : best), TEXT_SCALES[0]);
}

export function storedAllowance(override: number): string {
  return override ? String(override) : "";
}

export function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
