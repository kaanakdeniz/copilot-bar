
export type Client = "cli" | "vscode" | "app";

export interface Tokens {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
}

export interface SessionRef {
  id: string;
  label: string | null;
}

export interface UsageRecord {
  id: string;
  ts: number;
  client: Client;
  model: string;
  tokens: Tokens;
  aic: number | null;
  requests: number;
  project: string | null;
  session: SessionRef | null;
  initiator: string | null;
  reasoningEffort: string | null;
  isAuto: boolean | null;
}

export interface GitHubPlan {
  login: string;
  plan: string;
  sku: string | null;
  creditsUsed: number | null;
  entitlement: number | null;
  overagePermitted: boolean;
  unlimited: boolean;
  resetAt: number | null;
  fetchedAt: number;
}

export type PaceStatus = "noAllowance" | "under" | "onPace" | "ahead" | "exhausted";

export interface Pace {
  cycle: { start: number; end: number };
  used: number;
  loggedAic: number;
  unlogged: number;
  allowance: number | null;
  totalDays: number;
  elapsedDays: number;
  remainingDays: number;
  avgDaily: number;
  avgRecent: number;
  avgPerActiveDay: number;
  activeDays: number;
  projected: number;
  evenPace: number | null;
  delta: number | null;
  usedToday: number;
  lastHour: number;
  dailyBudget: number | null;
  runOutAt: number | null;
  status: PaceStatus;
  fracUsed: number | null;
  fracElapsed: number;
}

export type MenuBarDisplay = "icon-number" | "icon" | "number";
export type MenuBarIcon = "disc" | "ring" | "copilot" | "progress";
export type MenuBarNumber = "pct" | "used" | "left" | "used-total";
export interface MenuBarLook { display: MenuBarDisplay; icon: MenuBarIcon; number: MenuBarNumber }
export type RangeKey = "last7" | "last30" | "cycle" | "previousCycle" | "last90" | "all";

export interface Settings {
  openExpanded: boolean | null;
  cycleSummary: "gauge" | "bar";
  cycleHeadline: "used" | "remaining";
  menuBarDisplay: MenuBarDisplay | "";
  menuBarIcon: MenuBarIcon | "";
  menuBarNumber: MenuBarNumber | "";
  alertColors: boolean;
  theme: "system" | "light" | "dark";
  allowanceOverride: number;
  paceTolerancePct: number;
  weekStartsMonday: boolean;
  refreshMinutes: number;
  sources: { cli: boolean };
  cliLogDir: string | null;
  notifications: { enabled: boolean; thresholds: number[]; aheadOfPace: boolean; burstAic: number };
  onboarded: boolean;
  textScale: number;
  palette: "default" | "colorblind";
  hotkey: string;
  autoUpdateCheck: boolean;
}

export type Tab = "overview" | "models" | "projects" | "settings";

export type UpdateState =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "upToDate" }
  | { state: "available"; version: string; url: string }
  | { state: "downloading"; version: string }
  | { state: "installing"; version: string }
  | { state: "error"; message: string; url: string | null };

export type SignIn =
  | { state: "unknown" }
  | { state: "signedIn"; login: string }
  | { state: "ghMissing" }
  | { state: "signedOut" }
  | { state: "ssoRequired" }
  | { state: "accountMismatch"; ghLogin: string; copilotLogin: string }
  | { state: "error"; message: string };

export interface PlanError {
  message: string;
  offline: boolean;
}

export interface Status {
  lastRefresh: number | null;
  planError: PlanError | null;
  signIn: SignIn;
  cliFiles: number;
  connectingSince: number | null;
}

export type AllowanceSource = "custom" | "github" | "none";

export interface Snapshot {
  records: UsageRecord[];
  githubPlan: GitHubPlan | null;
  pace: Pace;
  allowanceSource: AllowanceSource;
  allowanceCapped: boolean;
  entitlementUsedUp: boolean;
  copilotStopped: boolean;
  overage: number;
  settings: Settings;
  status: Status;
  now: number;
}
