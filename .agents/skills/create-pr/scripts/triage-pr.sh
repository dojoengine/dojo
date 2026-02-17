#!/usr/bin/env bash
set -euo pipefail

pr=""
repo=""

usage() {
  cat <<'USAGE'
Usage: triage-pr.sh [--pr <number>] [--repo <owner/repo>]

Prints a single-shot summary of CI status, latest review, and latest comments.
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

checks_counts=$(gh pr checks "$pr" --repo "$repo" --json status,conclusion --jq '
  [length,
   (map(select(.status != "COMPLETED")) | length),
   (map(select(.conclusion != null and .conclusion != "SUCCESS")) | length),
   (map(select(.conclusion == "SUCCESS")) | length)
  ] | @tsv
' 2>/dev/null || true)

if [[ -n "$checks_counts" ]]; then
  IFS=$'\t' read -r total pending failed success <<< "$checks_counts"
  echo "CI: total=$total pending=$pending failed=$failed success=$success"
else
  echo "CI: unavailable"
fi

failed_checks=$(gh pr checks "$pr" --repo "$repo" --json name,conclusion,detailsUrl --jq '
  [.[] | select(.conclusion != null and .conclusion != "SUCCESS") |
    "\(.name)\t\(.conclusion)\t\(.detailsUrl // \"\")"
  ] | .[]
' 2>/dev/null || true)
if [[ -n "$failed_checks" ]]; then
  while IFS=$'\t' read -r name conclusion url; do
    if [[ -n "$name" ]]; then
      echo "FAIL: $name $conclusion $url"
    fi
  done <<< "$failed_checks"
fi

review_line=$(gh api "repos/$repo/pulls/$pr/reviews?per_page=100" --jq '
  [ .[] | select(.submitted_at != null) ] |
  if length == 0 then "" else
    (max_by(.submitted_at)) | "\(.state)\t\(.user.login)\t\(.submitted_at)\t\(.html_url)"
  end
' 2>/dev/null || true)
if [[ -n "$review_line" ]]; then
  IFS=$'\t' read -r r_state r_user r_time r_url <<< "$review_line"
  echo "REVIEW: $r_state $r_user $r_time $r_url"
fi

issue_line=$(gh api "repos/$repo/issues/$pr/comments?per_page=100" --jq '
  if length == 0 then "" else
    (max_by(.created_at)) | "\(.user.login)\t\(.created_at)\t\(.html_url)\t\(.body | gsub("\\n"; " ") | gsub("\\t"; " ") | .[0:200])"
  end
' 2>/dev/null || true)
if [[ -n "$issue_line" ]]; then
  IFS=$'\t' read -r c_user c_time c_url c_body <<< "$issue_line"
  echo "COMMENT: conversation $c_user $c_time $c_url $c_body"
fi

review_comment_line=$(gh api "repos/$repo/pulls/$pr/comments?per_page=100" --jq '
  if length == 0 then "" else
    (max_by(.created_at)) | "\(.user.login)\t\(.created_at)\t\(.html_url)\t\(.body | gsub("\\n"; " ") | gsub("\\t"; " ") | .[0:200])"
  end
' 2>/dev/null || true)
if [[ -n "$review_comment_line" ]]; then
  IFS=$'\t' read -r rc_user rc_time rc_url rc_body <<< "$review_comment_line"
  echo "COMMENT: inline $rc_user $rc_time $rc_url $rc_body"
fi
