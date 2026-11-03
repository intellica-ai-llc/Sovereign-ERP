#!/usr/bin/env bash
# Nightly backup of the VeriChain data directory to any rclone remote (Cloudflare R2, Backblaze B2, ...).
#   VC_DATA=/var/lib/vc-server BACKUP_REMOTE=r2:vc-backups [AGE_RECIPIENT=age1...] [KEEP=14] ./backup.sh
# The data directory holds the operator signing key: keep the bucket private and set AGE_RECIPIENT to encrypt.
# Cron: 17 3 * * *  /opt/verichain/backup.sh >> /var/log/vc-backup.log 2>&1
set -euo pipefail
: "${VC_DATA:?set VC_DATA}" "${BACKUP_REMOTE:?set BACKUP_REMOTE (an rclone remote:path)}"
KEEP="${KEEP:-14}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
out="$tmp/vc-data-$stamp.tar.gz"
# snapshot while running: files are append-only or replaced atomically, so a tar is consistent enough to restore
tar -C "$(dirname "$VC_DATA")" -czf "$out" "$(basename "$VC_DATA")"
if [ -n "${AGE_RECIPIENT:-}" ]; then age -r "$AGE_RECIPIENT" -o "$out.age" "$out"; out="$out.age"; fi
sum="$(sha256sum "$out" | cut -d' ' -f1)"
rclone copyto "$out" "$BACKUP_REMOTE/$(basename "$out")"
echo "$sum  $(basename "$out")" | rclone rcat "$BACKUP_REMOTE/$(basename "$out").sha256"
# verify the upload by size, then prune old backups (keep the newest $KEEP)
want="$(stat -c %s "$out")"; got="$(rclone size --json "$BACKUP_REMOTE/$(basename "$out")" | sed -n 's/.*"bytes":\([0-9]*\).*/\1/p')"
[ "$want" = "$got" ] || { echo "UPLOAD SIZE MISMATCH ($want vs $got)" >&2; exit 1; }
rclone lsf "$BACKUP_REMOTE" --include 'vc-data-*.tar.gz*' | grep -v '\.sha256$' | sort | head -n -"$KEEP" | while read -r f; do
  rclone deletefile "$BACKUP_REMOTE/$f"; rclone deletefile "$BACKUP_REMOTE/$f.sha256" 2>/dev/null || true
done
echo "backup ok: $(basename "$out") sha256 $sum"
