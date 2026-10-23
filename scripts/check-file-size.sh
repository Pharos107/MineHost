#!/usr/bin/env bash
# Chặn file mã nguồn dài quá giới hạn trong AGENTS.md. File sinh tự động được bỏ qua.
set -euo pipefail

MAX_LINES="${MAX_LINES:-500}"
cd "$(dirname "$0")/.."

failed=0
while IFS= read -r -d '' file; do
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$MAX_LINES" ]; then
    echo "::error file=$file::$file có $lines dòng (giới hạn $MAX_LINES). Hãy tách file."
    failed=1
  fi
done < <(find crates ui/src \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \) \
  -not -name 'schema.d.ts' -not -path '*/node_modules/*' -print0)

if [ "$failed" -eq 0 ]; then
  echo "Mọi file đều dưới $MAX_LINES dòng."
fi
exit "$failed"
