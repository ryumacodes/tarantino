#!/bin/sh
set -eu

blocked_name='aff''an'
absolute_user_path='/Users/[A-Za-z0-9._-]+'
exclude_self=':(exclude)scripts/check-repo-hygiene.sh'

fail=0

check_tree() {
  label="$1"
  shift

  if git grep "$@" -I -n -i "$blocked_name" -- . "$exclude_self"; then
    echo "error: found blocked username in $label" >&2
    fail=1
  fi

  if git grep "$@" -I -n -E "$absolute_user_path" -- . "$exclude_self"; then
    echo "error: found hardcoded local /Users path in $label" >&2
    fail=1
  fi
}

check_tree "working tree"
check_tree "staged changes" --cached

if [ "$fail" -ne 0 ]; then
  echo "Repo hygiene check failed. Remove local machine/user-specific paths before committing." >&2
  exit 1
fi

echo "Repo hygiene check passed."
