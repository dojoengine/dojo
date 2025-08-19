#!/usr/bin/env bash

set -Eeuo pipefail

# Validate that every version listed in versions.json exists as a Git tag or Release tag
# in the mapped GitHub repositories.
#
# Requirements:
#   - jq (JSON parsing)
#   - git (for ls-remote fallback)
#   - gh (GitHub CLI) optional but preferred (respects $GITHUB_TOKEN if set)

VERSION_REGISTRY_FILE="${1:-versions.json}"

# Component repository constants
KATANA_REPO="dojoengine/katana"
TORII_REPO="dojoengine/torii"

# Get the GitHub repository name for a given component.
#
# Arguments:
#   $1 - component name (e.g., "katana" or "torii")
#
# Returns:
#   GitHub repo in format "owner/repo" (e.g., "dojoengine/katana")
#
# Error:
#   Will fail if the provided component is unknown.
get_repo() {
	local component="$1"

	case "$component" in
		katana)
			echo "$KATANA_REPO"
			;;
		torii)
			echo "$TORII_REPO"
			;;
		*)
			echo "error: unknown component '$component'" >&2
			exit 1
			;;
	esac
}

have_gh=0
if command -v gh >/dev/null 2>&1; then
  have_gh=1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: `jq` is not installed." >&2
  exit 2
fi
if ! command -v git >/dev/null 2>&1; then
  echo "error: `git` is not installed" >&2
  exit 2
fi

if [[ ! -f "$VERSION_REGISTRY_FILE" ]]; then
  echo "error: cannot find $VERSION_REGISTRY_FILE" >&2
  exit 2
fi

# Simple de-dup cache to avoid re-querying same repo/tag
# Using a file-based approach instead of associative array
CACHE_FILE=$(mktemp)
trap "rm -f $CACHE_FILE" EXIT

missing=()

# Check if a Git tag exists in a GitHub repository.
# Uses caching to avoid redundant API/network calls for the same repo/tag combination.
#
# Arguments:
#   $1 - GitHub repository in format "owner/repo" (e.g., "dojoengine/katana")
#   $2 - Git tag name to check (e.g., "v1.5.0" or "1.5.0")
#
# Returns:
#   0 - if tag exists (success)
#   1 - if tag does not exist (failure)
#
# Side effects:
#   - Writes to cache file to store results
#   - Makes GitHub API calls (if gh CLI available) or git ls-remote calls
check_tag_exists() {
	local repo="$1"
	local tag="$2"
	local key="${repo}|${tag}"

	# Check cache to avoid redundant lookups
	if grep -q "^${key}=" "$CACHE_FILE" 2>/dev/null; then
		local result=$(grep "^${key}=" "$CACHE_FILE" | cut -d= -f2)
		return "$result"
	fi

	# Prefer gh API (release tag then git ref). Fall back to git ls-remote.
	if [[ $have_gh -eq 1 ]]; then
		# First check if it's a release tag
		if gh api -q . "repos/$repo/releases/tags/$tag" >/dev/null 2>&1; then
			echo "${key}=0" >> "$CACHE_FILE"
			return 0
		fi

		# Then check if it's a regular git tag
		if gh api -q . "repos/$repo/git/ref/tags/$tag" >/dev/null 2>&1; then
			echo "${key}=0" >> "$CACHE_FILE"
			return 0
		fi
	fi

	# Fallback using git ls-remote (no auth needed)
	if git ls-remote --tags "https://github.com/$repo" "refs/tags/$tag" \
		| grep -qE 'refs/tags/' ; then
		echo "${key}=0" >> "$CACHE_FILE"
		return 0
	fi

	# Tag not found, cache the negative result
	echo "${key}=1" >> "$CACHE_FILE"
	return 1
}

echo "Validating versions listed in $VERSION_REGISTRY_FILE ..."

# We need to track the current version being validated
current_version=""

# get all the dojo versions in the registry
all_versions=$(jq -r 'keys_unsorted[]' $VERSION_REGISTRY_FILE)

# iterate over all the dojo versions
for version in $all_versions; do
	echo
	echo -e "Validating version: \033[1m$version\033[0m"

	# get the components for the current version
	comps=$(jq -r ".[\"${version}\"] | keys_unsorted[]" $VERSION_REGISTRY_FILE)

	# iterate over all the components for each version
	for comp in $comps; do
		repo=$(get_repo "$comp")
		echo "  • $comp (repo: $repo)"

		# iterate over the current component's versions
		comp_versions=$(jq -r ".\"${version}\".${comp}[]" $VERSION_REGISTRY_FILE)
		for comp_version in $comp_versions; do
			# check both with and without the v-prefix
			found=0
			for tag in "v${comp_version}" "${comp_version}"; do
				if check_tag_exists "$repo" "$tag"; then
					echo -e "    \033[32m✓\033[0m $comp_version"
					found=1; break
				fi
			done

			if [[ $found -eq 0 ]]; then
				echo -e "    \033[31m✗\033[0m $comp_version"
				missing+=("$comp $comp_version")
			fi
		done
	done
done

echo
if (( ${#missing[@]} > 0 )); then
	echo "Missing versions:"
	printf ' - %s\n' "${missing[@]}"
	exit 1
fi

echo "All listed component versions exist ✅"
