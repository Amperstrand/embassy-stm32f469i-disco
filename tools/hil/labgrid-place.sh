#!/usr/bin/env bash
# Idempotently create the BSP F469 HIL place with its resource match.
# The token comes from the microfips bench exporter — the same stlink the
# micronuts-qr-rig and gm65-qr-loopback places bind: acquiring this place
# EXCLUDES those sessions (and vice versa), which is the cross-project
# exclusivity contract. Direct I/O stays the working path (BenchLock first,
# then the place — never reversed; playbook pattern 12).
set -euo pipefail

COORDINATOR="${LABGRID_COORDINATOR:-192.168.13.221:20408}"
EXPORTER_NAME="${LABGRID_EXPORTER_NAME:-ai-legion-small-microfips}"
PLACE="bsp-f469-hil"
LG="${LABGRID_CLIENT:-labgrid-client}"

lg() { "$LG" -x "$COORDINATOR" -p "$PLACE" "$@"; }

if lg show >/dev/null 2>&1; then
    echo "place $PLACE exists"
else
    lg create
    echo "place $PLACE created"
fi

lg add-match "${EXPORTER_NAME}/stm32-stlink/BenchSerialToken"
lg set-comment "BSP HIL (run_hil.sh phases) on the shared F469 — image backup/restore around every session (embassy-stm32f469i-disco tools/hil)"
lg set-tags "firmware=-" "test=bsp-hil" "owner=embassy-stm32f469i-disco" "ts=$(date +%Y%m%dT%H%M%S)"
lg show | sed -n '1,8p'
