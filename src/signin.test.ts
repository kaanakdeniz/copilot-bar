import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SignInStatus, signInCopy } from "./components";
import type { SignIn } from "./types";

const text = (signIn: SignIn) => renderToStaticMarkup(createElement(SignInStatus, { signIn })).replace(/<[^>]+>/g, "").replace(/&#x27;|&#39;/g, "'");

describe("GitHub CLI sign-in states and their next step", () => {
  it("signed in: names the account, nothing to do", () => {
    const t = text({ state: "signedIn", login: "octo" });
    expect(t).toContain("Signed in as @octo");
    expect(t).not.toMatch(/gh auth|brew/);
  });
  it("gh not installed: install it with Homebrew, then sign in", () => {
    const t = text({ state: "ghMissing" });
    expect(t).toContain("GitHub CLI isn't installed");
    expect(t).toContain("brew install gh");
    expect(t).toContain("gh auth login");
  });
  it("signed out: sign in with gh", () => {
    const t = text({ state: "signedOut" });
    expect(t).toContain("GitHub CLI isn't signed in");
    expect(t).toContain("gh auth login");
  });
  it("SSO not authorised: refresh the sign-in and authorise the organisation", () => {
    const t = text({ state: "ssoRequired" });
    expect(t).toContain("SSO");
    expect(t).toContain("gh auth refresh");
    expect(t).toContain("Authorize");
  });
  it("account mismatch: switch gh to the Copilot CLI's account", () => {
    const t = text({ state: "accountMismatch", ghLogin: "octo", copilotLogin: "octo-work" });
    expect(t).toContain("@octo");
    expect(t).toContain("@octo-work");
    expect(t).toContain("gh auth switch --user octo-work");
  });
  it("any other error: says what happened and how to check gh", () => {
    const t = text({ state: "error", message: "GitHub returned 502: Server Error" });
    expect(t).toContain("GitHub returned 502: Server Error");
    expect(t).toContain("gh auth status");
  });
  it("not checked yet", () => {
    expect(text({ state: "unknown" })).toContain("Checking the GitHub CLI");
  });
  it("every state notes that local usage keeps working without plan data", () => {
    for (const s of [{ state: "ghMissing" }, { state: "signedOut" }, { state: "ssoRequired" }, { state: "error", message: "x" }] as SignIn[]) {
      expect(text(s)).toContain("Local usage still works");
    }
  });
});

describe("the short sign-in line in the menu bar popover's footer", () => {
  it("names the problem; the user fixes sign-in problems, GitHub errors are retried", () => {
    const short = (signIn: SignIn) => { const c = signInCopy(signIn); return [c.short, c.problem]; };
    expect(short({ state: "ghMissing" })).toEqual(["GitHub CLI not installed", "fix"]);
    expect(short({ state: "signedOut" })).toEqual(["GitHub CLI not signed in", "fix"]);
    expect(short({ state: "ssoRequired" })).toEqual(["GitHub CLI needs SSO", "fix"]);
    expect(short({ state: "accountMismatch", ghLogin: "octo", copilotLogin: "octo-work" })).toEqual(["Plan data is @octo's, not @octo-work's", "fix"]);
    expect(short({ state: "error", message: "GitHub returned 502: Server Error" })).toEqual(["GitHub error", "retry"]);
    expect(short({ state: "signedIn", login: "octo" })).toEqual(["@octo", false]);
    expect(short({ state: "unknown" })[1]).toBe(false);
  });
});
