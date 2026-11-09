#!/usr/bin/env bash
# Saves a compressed copy of the whole database into deploy/backups/ and keeps
# the newest 30. Run it from anywhere:  ./deploy/backup.sh
# Schedule it daily (see README), and copy the backups OFF this server.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p backups
file="backups/amber-$(date +%Y%m%d-%H%M%S).sql.gz"

docker compose exec -T db pg_dump -U amber --clean --if-exists amber | gzip > "$file"

# a backup that is empty or corrupt is worse than none: refuse it
if ! gzip -t "$file" || [ "$(wc -c < "$file")" -lt 1000 ]; then
  rm -f "$file"
  echo "BACKUP FAILED: the file was empty or damaged" >&2
  exit 1
fi

ls -1t backups/amber-*.sql.gz | tail -n +31 | xargs -r rm --
echo "saved $file ($(wc -c < "$file") bytes)"
