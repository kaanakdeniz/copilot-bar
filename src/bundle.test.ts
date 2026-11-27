import { build, type Rollup } from "vite";
import { describe, expect, it, vi } from "vitest";

const DEV_ONLY = ["studio-config", "Menu bar icon lab", "octocat", "Mostly web usage", "Purple (yours)", "copilotbar.studio"];

describe("production bundle", () => {
  it("contains no dev-only tooling or demo data", async () => {
    vi.stubEnv("NODE_ENV", "production");
    try {
      const result = await build({ logLevel: "silent", build: { write: false } });
      const outputs = (Array.isArray(result) ? result : [result]) as Rollup.RollupOutput[];
      const js = outputs.flatMap((o) => o.output).map((c) => (c.type === "chunk" ? c.code : "")).join("\n");
      expect(js.length).toBeGreaterThan(0);
      for (const s of DEV_ONLY) expect(js.includes(s), s).toBe(false);
    } finally {
      vi.unstubAllEnvs();
    }
  }, 120_000);
});
