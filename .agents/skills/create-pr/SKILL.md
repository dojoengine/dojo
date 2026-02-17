---
name: create-pr
description: Create or update a PR from current branch to main, watch CI, and address feedback
---
The user likes the state of the code.

There are $`git status --porcelain | wc -l | tr -d ' '` uncommitted changes.
The current branch is $`git branch --show-current`.
The target branch is origin/main.

$`git rev-parse --abbrev-ref @{upstream} 2>/dev/null && echo "Upstream branch exists." || echo "There is no upstream branch yet."`

**Existing PR:** $`gh pr view --json number,title,url --jq '"#\(.number): \(.title) - \(.url)"' 2>/dev/null || echo "None"`

The user requested a PR.

Follow these exact steps:

## Phase 1: Review the code

1. Review test coverage
2. Check for silent failures
3. Verify code comments are accurate
4. Review any new types
5. General code review

## Phase 2: Create/Update PR

6. Run `git diff` to review uncommitted changes
7. Commit them. Follow any instructions the user gave you about writing commit messages.
8. Push to origin.
9. Use `git diff origin/main...` to review the full PR diff
10. Check if a PR already exists for this branch:
   - **If PR exists**:
     - Draft/update the description in a temp file (e.g. `/tmp/pr-body.txt`).
     - Update the PR body using the non-deprecated script:
       - `./.agents/skills/create-pr/scripts/pr-body-update.sh --file /tmp/pr-body.txt`
     - Re-fetch the body with `gh pr view --json body --jq .body` to confirm it changed.
   - **If no PR exists**: Use `gh pr create --base main` to create a new PR. Keep the title under 80 characters and the description under five sentences.

The PR description should summarize ALL commits in the PR, not just the latest changes.

## Phase 3: Monitor CI and Address Issues

Note: Keep commands CI-safe and avoid interactive `gh` prompts. Ensure `GH_TOKEN` or `GITHUB_TOKEN` is set in CI.

11. Watch CI status and feedback using the polling script (instead of running `gh` in a loop):
   - Run `./.agents/skills/create-pr/scripts/poll-pr.sh --triage-on-change --exit-when-green` (polls every 30s for 10 mins).
   - If checks fail, use `gh pr checks` or `gh run list` to find the failing run id, then:
     - Fetch the failed check logs using `gh run view <run-id> --log-failed`
     - Analyze the failure and fix the issue
     - Commit and push the fix
     - Continue polling until all checks pass

12. Check for merge conflicts:
   - Run `git fetch origin main && git merge origin/main`
   - If conflicts exist, resolve them sensibly
   - Commit the merge resolution and push

13. Use the polling script output to notice new reviews and comments (avoid direct polling via `gh`):
   - If you need a full snapshot, run `./.agents/skills/create-pr/scripts/triage-pr.sh` once.
   - If you need full context after the script reports a new item, fetch details once with `gh pr view --comments` or `gh api ...`.
   - **Address feedback**:
     - For bot reviews, read the review body and any inline comments carefully
     - Address comments that are clearly actionable (bug fixes, typos, simple improvements)
     - Skip comments that require design decisions or user input
     - For addressed feedback, commit fixes with a message referencing the review/comment

## Phase 4: Merge and Cleanup

14. Once CI passes and the PR is approved, ask the user if they want to merge the PR.

15. If the user confirms, merge the PR:
    - Use `gh pr merge --squash --delete-branch` to squash-merge and delete the remote branch

16. After successful merge, check if we're in a git worktree:
    - Run: `[ "$(git rev-parse --git-common-dir)" != "$(git rev-parse --git-dir)" ]`
    - **If in a worktree**: Use the ask user question tool (`request_user_input`) to ask if they want to clean up the worktree. If yes, run `wt remove --yes --force` to remove the worktree and local branch, then switch back to the main worktree.
    - **If not in a worktree**: Just switch back to main with `git checkout main && git pull`

## Completion

Report the final PR status to the user, including:
- PR URL
- CI status (passed/merged)
- Any unresolved review comments that need user attention
- Cleanup status (worktree removed or branch switched)

If any step fails in a way you cannot resolve, ask the user for help.
