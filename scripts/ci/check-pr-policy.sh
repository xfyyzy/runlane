#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/ci/check-pr-policy.sh <pr-body-file>" >&2
  exit 2
fi

body_file="$1"
if [[ ! -r "$body_file" ]]; then
  echo "::error title=PR policy::PR body file is not readable: $body_file" >&2
  exit 1
fi

failures=0

fail() {
  echo "::error title=PR policy::$1" >&2
  failures=1
}

require_pattern() {
  local message="$1"
  local pattern="$2"
  if ! grep -Eiq "$pattern" "$body_file"; then
    fail "$message"
  fi
}

require_heading() {
  local label="$1"
  local pattern="$2"
  require_pattern "Missing required PR section: $label" "$pattern"
}

require_pattern \
  "PR body must link an issue with a closing keyword such as Closes #123" \
  '(^|[^[:alnum:]_])(close[sd]?|fix(e[sd])?|resolve[sd]?)[[:space:]]+#[0-9]+'

require_heading "What Changed" '^##[[:space:]]+What[[:space:]]+Changed[[:space:]]*$'
require_heading "Verification" '^##[[:space:]]+Verification[[:space:]]*$'
require_heading "Self-Review" '^##[[:space:]]+Self-Review[[:space:]]*$'
require_heading "Risks / Follow-Up" '^##[[:space:]]+Risks[[:space:]]*/[[:space:]]*Follow[- ]?Up[[:space:]]*$'

self_review="$(
  awk '
    BEGIN { in_self_review = 0 }
    /^##[[:space:]]+Self-Review[[:space:]]*$/ { in_self_review = 1; next }
    /^##[[:space:]]+/ && in_self_review { in_self_review = 0 }
    in_self_review { print }
  ' "$body_file"
)"

if ! printf '%s\n' "$self_review" | grep -Eq '^[[:space:]]*-[[:space:]]+\[[ xX]\]'; then
  fail "Self-Review section must contain a markdown checklist"
elif ! printf '%s\n' "$self_review" | grep -Eq '^[[:space:]]*-[[:space:]]+\[[xX]\]'; then
  fail "Self-Review checklist must have at least one checked item"
fi

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

echo "PR policy ok: linked issue, required sections, and checked self-review item present"
