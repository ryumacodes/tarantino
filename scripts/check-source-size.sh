#!/usr/bin/env bash
set -euo pipefail

default_limit=700
exceptions_file="scripts/source-size-exceptions.txt"
failed=0

while IFS= read -r -d '' file; do
  [[ -f "$file" ]] || continue
  lines=$(wc -l < "$file")
  ceiling=$default_limit
  reason=""

  if [[ -f "$exceptions_file" ]]; then
    exception=$(awk -F '|' -v path="$file" '$1 == path { print $2 "|" $3; exit }' "$exceptions_file")
    if [[ -n "$exception" ]]; then
      reason=${exception%|*}
      ceiling=${exception##*|}
    fi
  fi

  if (( lines > ceiling )); then
    echo "error: $file has $lines lines (limit: $ceiling)" >&2
    failed=1
  fi
done < <(git ls-files -co --exclude-standard -z -- \
  '*.rs' '*.mm' '*.c' '*.h' '*.cpp' '*.wgsl' '*.ts' '*.tsx' '*.js' '*.jsx' '*.css' '*.sh' '*.py')

if (( failed != 0 )); then
  echo "Source-size check failed." >&2
  exit 1
fi

echo "Source-size check passed (limit: $default_limit; documented exceptions apply)."
