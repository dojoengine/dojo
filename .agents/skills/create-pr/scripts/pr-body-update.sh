#!/usr/bin/env bash
set -euo pipefail

body_file=""
pr=""
repo=""

usage() {
  cat <<'USAGE'
Usage: pr-body-update.sh --file <path> [--pr <number>] [--repo <owner/repo>]

Updates a PR body using the GraphQL updatePullRequest mutation and verifies the result.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file)
      body_file="$2"
      shift 2
      ;;
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

if [[ -z "$body_file" ]]; then
  echo "--file is required." >&2
  exit 1
fi

if [[ ! -f "$body_file" ]]; then
  echo "Body file not found: $body_file" >&2
  exit 1
fi

if [[ ! -s "$body_file" ]]; then
  echo "Body file is empty: $body_file" >&2
  exit 1
fi

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

pr_id="$(gh pr view "$pr" --repo "$repo" --json id --jq .id 2>/dev/null || true)"
if [[ -z "$pr_id" ]]; then
  echo "Could not determine PR id for #$pr in $repo." >&2
  exit 1
fi

gh api graphql \
  -f query='mutation($id:ID!,$body:String!){updatePullRequest(input:{pullRequestId:$id, body:$body}){pullRequest{id}}}' \
  -f id="$pr_id" \
  -f body="$(cat "$body_file")" \
  >/dev/null

updated_body="$(gh pr view "$pr" --repo "$repo" --json body --jq .body 2>/dev/null || true)"
if [[ -z "$updated_body" ]]; then
  echo "Failed to fetch updated PR body for #$pr in $repo." >&2
  exit 1
fi

if [[ "$updated_body" != "$(cat "$body_file")" ]]; then
  echo "PR body mismatch after update." >&2
  exit 1
fi

echo "Updated PR #$pr body in $repo."
