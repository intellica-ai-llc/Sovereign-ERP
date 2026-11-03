#!/usr/bin/env bash
# One witnessing round: fetch the log's checkpoint + consistency proof, have the witness cosign, post the cosignature back.
# Needs: curl, jq.  Env: LOG_URL, LOG_KEY (tenant API key), WITNESS_URL, STATE (file holding the witness's last size).
# Run it from cron or a loop (every minute is plenty). Exit 0 = cosigned or nothing new.
set -euo pipefail
: "${LOG_URL:?}" "${LOG_KEY:?}" "${WITNESS_URL:?}"
STATE="${STATE:-./witness-last-size}"
old=$(cat "$STATE" 2>/dev/null || echo 0)
for attempt in 1 2; do
  cons=$(curl -fsS -H "Authorization: Bearer $LOG_KEY" "$LOG_URL/v1/checkpoint/consistency?from=$old")
  size=$(jq -r .tree_size <<<"$cons")
  if [ "$size" = "$old" ]; then echo "nothing new (size $size)"; exit 0; fi
  body="old $old"$'\n'
  while read -r h; do [ -n "$h" ] && body+="$h"$'\n'; done < <(jq -r '.proof_b64[]' <<<"$cons")
  body+=$'\n'"$(jq -r .checkpoint <<<"$cons")"
  code=$(curl -sS -o /tmp/witness-resp.$$ -w '%{http_code}' -X POST --data-binary "$body" "$WITNESS_URL/add-checkpoint")
  if [ "$code" = 409 ]; then old=$(cat /tmp/witness-resp.$$); rm -f /tmp/witness-resp.$$; continue; fi   # witness knows a different size: retry with its size
  if [ "$code" != 200 ]; then echo "witness refused ($code): $(cat /tmp/witness-resp.$$)" >&2; rm -f /tmp/witness-resp.$$; exit 1; fi
  line=$(cat /tmp/witness-resp.$$; printf x); line="${line%x}"; rm -f /tmp/witness-resp.$$
  payload=$(jq -n --argjson size "$size" --arg line "$line" '{size:$size,line:$line}')
  curl -fsS -X POST -H "Authorization: Bearer $LOG_KEY" -H 'Content-Type: application/json' -d "$payload" "$LOG_URL/v1/cosignatures" >/dev/null
  echo "$size" > "$STATE"; echo "cosigned size $size"; exit 0
done
echo "could not agree on an old size" >&2; exit 1
