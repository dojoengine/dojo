#!/usr/bin/env bash
set -euo pipefail

interval="${POLL_INTERVAL:-30}"
minutes="${POLL_MINUTES:-10}"
poll_once="${POLL_ONCE:-0}"
pr=""
repo=""
exit_when_green=0
triage_on_change=0

usage() {
  cat <<'USAGE'
Usage: poll-pr.sh [--pr <number>] [--repo <owner/repo>] [--interval <seconds>] [--minutes <minutes>] [--exit-when-green] [--triage-on-change]

Polls PR checks, review comments, and conversation comments every 30s for 10 minutes by default.
Environment overrides: POLL_INTERVAL, POLL_MINUTES, POLL_ONCE=1.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pr)
      pr="$2"
      shift 2
      ;;
    --repo)
      repo="$2"
      shift 2
      ;;
    --interval)
      interval="$2"
      shift 2
      ;;
    --minutes)
      minutes="$2"
      shift 2
      ;;
    --exit-when-green)
      exit_when_green=1
      shift 1
      ;;
    --triage-on-change)
      triage_on_change=1
      shift 1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$pr" ]]; then
  pr="$(gh pr view --json number --jq .number 2>/dev/null || true)"
fi

if [[ -z "$pr" ]]; then
  echo "Could not determine PR number. Use --pr <number>." >&2
  exit 1
fi

if [[ -z "$repo" ]]; then
  repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)"
fi

if [[ -z "$repo" ]]; then
  echo "Could not determine repo. Use --repo owner/name." >&2
  exit 1
fi

if ! [[ "$interval" =~ ^[0-9]+$ && "$minutes" =~ ^[0-9]+$ ]]; then
  echo "interval and minutes must be integers." >&2
  exit 1
fi

iterations=$(( (minutes * 60) / interval ))
if (( iterations < 1 )); then
  iterations=1
fi
if [[ "$poll_once" == "1" ]]; then
  iterations=1
fi

echo "Polling PR #$pr in $repo every ${interval}s for ${minutes}m (${iterations} iterations)."

last_issue_comment_id=""
last_review_comment_id=""
last_review_id=""
last_failed_signature=""

if ! gh auth status >/dev/null 2>&1; then
  if [[ -z "${GITHUB_TOKEN:-}" && -z "${GH_TOKEN:-}" ]]; then
    echo "Warning: gh auth not configured (set GH_TOKEN or GITHUB_TOKEN)."
  fi
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

print_new() {
  local kind="$1"
  local time="$2"
  local user="$3"
  local url="$4"
  local body="$5"

  echo "New $kind by @$user at $time"
  if [[ -n "$body" ]]; then
    echo "  $body"
  fi
  if [[ -n "$url" ]]; then
    echo "  $url"
  fi
}

for i in $(seq 1 "$iterations"); do
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  echo "[$now] Poll $i/$iterations"
  changed=0

  checks_counts=$(gh pr checks "$pr" --repo "$repo" --json status,conclusion --jq '
    [length,
     (map(select(.status != "COMPLETED")) | length),
     (map(select(.conclusion != null and .conclusion != "SUCCESS")) | length),
     (map(select(.conclusion == "SUCCESS")) | length)
    ] | @tsv
  ' 2>/dev/null || true)
  if [[ -n "$checks_counts" ]]; then
    IFS=$'\t' read -r total pending failed success <<< "$checks_counts"
    echo "Checks: total=$total pending=$pending failed=$failed success=$success"
  else
    echo "Checks: unavailable"
  fi

  failed_checks=$(gh pr checks "$pr" --repo "$repo" --json name,conclusion,detailsUrl --jq '
    [.[] | select(.conclusion != null and .conclusion != "SUCCESS") |
      "\(.name) (\(.conclusion))\t\(.detailsUrl // \"\")"
    ] | .[]
  ' 2>/dev/null || true)
  failed_signature=$(gh pr checks "$pr" --repo "$repo" --json name,conclusion --jq '
    [.[] | select(.conclusion != null and .conclusion != "SUCCESS") |
      "\(.name):\(.conclusion)"
    ] | sort | join("|")
  ' 2>/dev/null || true)

  if [[ -n "$failed_checks" ]]; then
    echo "Failed checks:"
    while IFS=$'\t' read -r name url; do
      if [[ -n "$name" ]]; then
        if [[ -n "$url" ]]; then
          echo "  - $name $url"
        else
          echo "  - $name"
        fi
      fi
    done <<< "$failed_checks"
  fi
  if [[ -n "$failed_signature" && "$failed_signature" != "$last_failed_signature" ]]; then
    last_failed_signature="$failed_signature"
    changed=1
  fi

  issue_line=$(gh api "repos/$repo/issues/$pr/comments?per_page=100" --jq '
    if length == 0 then "" else
      (max_by(.created_at)) | "\(.id)\t\(.created_at)\t\(.user.login)\t\(.html_url)\t\(.body | gsub("\\n"; " ") | gsub("\\t"; " ") | .[0:200])"
    end
  ' 2>/dev/null || true)
  if [[ -n "$issue_line" ]]; then
    IFS=$'\t' read -r issue_id issue_time issue_user issue_url issue_body <<< "$issue_line"
    if [[ "$issue_id" != "$last_issue_comment_id" ]]; then
      last_issue_comment_id="$issue_id"
      print_new "conversation comment" "$issue_time" "$issue_user" "$issue_url" "$issue_body"
      changed=1
    fi
  fi

  review_comment_line=$(gh api "repos/$repo/pulls/$pr/comments?per_page=100" --jq '
    if length == 0 then "" else
      (max_by(.created_at)) | "\(.id)\t\(.created_at)\t\(.user.login)\t\(.html_url)\t\(.body | gsub("\\n"; " ") | gsub("\\t"; " ") | .[0:200])"
    end
  ' 2>/dev/null || true)
  if [[ -n "$review_comment_line" ]]; then
    IFS=$'\t' read -r rc_id rc_time rc_user rc_url rc_body <<< "$review_comment_line"
    if [[ "$rc_id" != "$last_review_comment_id" ]]; then
      last_review_comment_id="$rc_id"
      print_new "inline review comment" "$rc_time" "$rc_user" "$rc_url" "$rc_body"
      changed=1
    fi
  fi

  review_line=$(gh api "repos/$repo/pulls/$pr/reviews?per_page=100" --jq '
    [ .[] | select(.submitted_at != null) ] |
    if length == 0 then "" else
      (max_by(.submitted_at)) | "\(.id)\t\(.submitted_at)\t\(.user.login)\t\(.html_url)\t\(.state)\t\(.body | gsub("\\n"; " ") | gsub("\\t"; " ") | .[0:200])"
    end
  ' 2>/dev/null || true)
  if [[ -n "$review_line" ]]; then
    IFS=$'\t' read -r r_id r_time r_user r_url r_state r_body <<< "$review_line"
    if [[ "$r_id" != "$last_review_id" ]]; then
      last_review_id="$r_id"
      print_new "review ($r_state)" "$r_time" "$r_user" "$r_url" "$r_body"
      changed=1
    fi
  fi

  if [[ "$triage_on_change" == "1" && "$changed" == "1" ]]; then
    "$script_dir/triage-pr.sh" --pr "$pr" --repo "$repo" || true
  fi

  if [[ "$exit_when_green" == "1" && -n "${pending:-}" ]]; then
    if (( pending == 0 && failed == 0 && total > 0 )); then
      echo "Checks green; exiting early."
      break
    fi
  fi

  if (( i < iterations )); then
    sleep "$interval"
  fi
done
