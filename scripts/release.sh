#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version>

Updates Cargo.toml, refreshes Cargo.lock with cargo check, verifies the lockfile
with --locked, commits the release preparation, creates an annotated tag, and
pushes the current branch and tag to origin.

Example:
  scripts/release.sh 0.1.2
  scripts/release.sh 0.2.0-rc.1
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_clean_worktree() {
  if [[ -n "$(git status --porcelain)" ]]; then
    fail "worktree must be clean before starting a release"
  fi
}

current_version() {
  perl -ne 'if (/^version = "([^"]+)"$/) { print "$1\n"; exit }' Cargo.toml
}

update_package_version() {
  local version="$1"
  local temp_file

  temp_file="$(mktemp)"

  awk -v version="$version" '
    BEGIN {
      in_package = 0
      updated = 0
    }

    /^\[package\]$/ {
      in_package = 1
      print
      next
    }

    /^\[/ && $0 != "[package]" {
      in_package = 0
    }

    in_package && /^version = "/ && !updated {
      print "version = \"" version "\""
      updated = 1
      next
    }

    {
      print
    }

    END {
      if (!updated) {
        exit 1
      }
    }
  ' Cargo.toml > "$temp_file" || {
    rm -f "$temp_file"
    fail "failed to update package version in Cargo.toml"
  }

  mv "$temp_file" Cargo.toml
}

main() {
  local branch current tag version

  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
  fi

  if [[ $# -ne 1 ]]; then
    usage >&2
    exit 1
  fi

  version="$1"
  if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    fail "version must look like 0.1.2 or 0.2.0-rc.1"
  fi

  git rev-parse --show-toplevel >/dev/null 2>&1 || fail "must be run inside a git repository"
  cd "$(git rev-parse --show-toplevel)"

  [[ -f Cargo.toml ]] || fail "Cargo.toml not found at repository root"
  [[ -f Cargo.lock ]] || fail "Cargo.lock not found at repository root"

  git remote get-url origin >/dev/null 2>&1 || fail "origin remote is required"

  branch="$(git branch --show-current)"
  [[ -n "$branch" ]] || fail "must be on a branch to push release commit"

  require_clean_worktree

  current="$(current_version)"
  [[ -n "$current" ]] || fail "failed to read current package version"
  [[ "$current" != "$version" ]] || fail "Cargo.toml is already at version $version"

  tag="v$version"

  if git rev-parse --verify "$tag" >/dev/null 2>&1; then
    fail "tag $tag already exists locally"
  fi

  if git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1; then
    fail "tag $tag already exists on origin"
  fi

  printf 'Updating Cargo.toml to %s\n' "$version"
  update_package_version "$version"

  printf 'Running cargo check to refresh Cargo.lock\n'
  cargo check --all-targets

  printf 'Verifying lockfile with --locked\n'
  cargo check --locked --all-targets

  printf 'Creating release commit\n'
  git add Cargo.toml Cargo.lock
  git commit -m "chore: prepare release $version"

  printf 'Creating annotated tag %s\n' "$tag"
  git tag -a "$tag" -m "Timelocked $tag"

  printf 'Pushing %s and %s to origin\n' "$branch" "$tag"
  git push origin "$branch"
  git push origin "$tag"
}

main "$@"
