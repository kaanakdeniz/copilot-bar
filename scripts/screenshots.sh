#!/usr/bin/env bash
# Demo scenarios only: real usage data must never end up in the committed screenshots.
set -euo pipefail
OUT="${1:-docs/screenshots}"
mkdir -p "$OUT"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
BASE="http://localhost:1420/"
shot() {
  local name="$1" query="$2" width="$3" height="$4"
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --force-prefers-reduced-motion \
    --virtual-time-budget=4000 --window-size="$width,$height" --screenshot="$OUT/$name.png" "$BASE?$query" >/dev/null 2>&1
  echo "$OUT/$name.png"
}
shot popover-light            "view=popover&demo=onpace&theme=light"                  760 460
shot popover-dark             "view=popover&demo=ahead&theme=dark"                    760 460
shot popover-expanded-light   "view=popover&demo=onpace&mode=advanced&theme=light"    760 800
shot popover-calendar-dark    "view=popover&demo=ahead&mode=calendar&theme=dark"      760 800
shot dashboard-overview-light "view=dashboard&demo=ahead&tab=overview&theme=light"   1200 1300
shot dashboard-overview-dark  "view=dashboard&demo=ahead&tab=overview&theme=dark"    1200 1300
shot dashboard-models-light   "view=dashboard&demo=onpace&tab=models&theme=light"    1200 340
shot dashboard-projects-dark  "view=dashboard&demo=onpace&tab=projects&theme=dark"   1200 960
shot dashboard-settings-light "view=dashboard&demo=onpace&tab=settings&theme=light"  1200 1600
