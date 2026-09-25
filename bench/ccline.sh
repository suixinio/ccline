#!/bin/sh
# POSIX shell equivalent of the ccline Rust binary.
# Reads Claude Code status JSON from stdin, outputs ANSI-formatted status line.
set -eu

eval "$(cat | jq -r '
  "cwd=\(.workspace.current_dir | @sh) " +
  "model=\(.model.display_name | @sh) " +
  "effort=\((.effort.level // "") | @sh) " +
  "cost=\(.cost.total_cost_usd) " +
  "pct=\(.context_window.used_percentage) " +
  "week_pct=\((.rate_limits.seven_day.used_percentage // "") | @sh) " +
  "week_reset=\((.rate_limits.seven_day.resets_at // "") | @sh)"
')"
last_two=$(echo "$cwd" | rev | cut -d/ -f1-2 | rev)

pct_fmt=$(printf '%.0f' "$pct")
cost_fmt=$(printf '$%.2f' "$cost")

# Colors (Monokai Pro ~60%)
GREEN="\033[38;2;122;158;86m"
CYAN="\033[38;2;90;158;160m"
PURPLE="\033[38;2;122;109;176m"
YELLOW="\033[38;2;176;154;66m"
LGRAY="\033[37m"
GRAY="\033[90m"
RST="\033[0m"

SEP=" ${GRAY}|${RST} "

git_info=""
if git -C "$cwd" rev-parse --git-dir >/dev/null 2>&1; then
  branch=$(git -C "$cwd" --no-optional-locks branch --show-current 2>/dev/null || echo "")
  if [ -n "$branch" ]; then
    if ! git -C "$cwd" --no-optional-locks diff --quiet 2>/dev/null || \
       ! git -C "$cwd" --no-optional-locks diff --cached --quiet 2>/dev/null || \
       [ -n "$(git -C "$cwd" --no-optional-locks ls-files --others --exclude-standard 2>/dev/null)" ]; then
      dirty="*"
    else
      dirty=""
    fi
    git_info="${SEP}${PURPLE}${branch}${dirty}${RST}"
  fi
fi

effort_suffix=""
if [ -n "$effort" ]; then
  effort_suffix=" ${GRAY}(${RST}${YELLOW}${effort}${RST}${GRAY})${RST}"
fi

week_info=""
if [ -n "$week_pct" ] && [ -n "$week_reset" ]; then
  left=$((${week_reset%.*} - $(date +%s)))
  if [ "$left" -gt 0 ]; then
    d=$((left / 86400)); h=$((left % 86400 / 3600)); m=$((left % 3600 / 60))
    if [ "$d" -gt 0 ]; then dur="${d}d${h}h"
    elif [ "$h" -gt 0 ]; then dur="${h}h${m}m"
    else dur="${m}m"; fi
    week_info="${SEP}${CYAN}$(printf '%.0f' "$week_pct")%%/7d ↻${dur}${RST}"
  fi
fi

printf "${GREEN}${model}${RST}${effort_suffix}${SEP}${CYAN}${last_two}${RST}${git_info}${SEP}${YELLOW}${pct_fmt}%% ctx${RST}${SEP}${LGRAY}${cost_fmt}${RST}${week_info}"
