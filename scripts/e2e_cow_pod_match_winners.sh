#!/usr/bin/env bash

COW="/tmp/cow.$$"
LOCAL="/tmp/local.$$"

# Read stdin into a temp file (so we can grep twice)
cat > /tmp/log_input.$$
LOG="/tmp/log_input.$$"

# Extract Autopilot winner
grep "\[pod\] CoW winner selected" "$LOG" \
| sed -nE 's/.*auction_id=([0-9]+).*submission_address=([^[:space:]]+).*computed_score=Some\(Score\(Ether\(([0-9]+)\)\)\).*/\1 \2 \3/p' \
| sort -k1,1 > "$COW"

# Extract solvers winners
grep "\[pod\] local winner selected" "$LOG" \
| sed -nE 's/.*auction_id=([0-9]+).*submission_address=([^[:space:]]+).*computed_score=Some\(Score\(Ether\(([0-9]+)\)\)\).*/\1 \2 \3/p' \
| sort -k1,1 > "$LOCAL"


echo "Checking winner consistency..."
echo

# 1) Detect duplicate Autopilot winners
if awk '{count[$1]++} END {for (i in count) if (count[i] > 1) exit 1}' "$COW"; then
  true
else
  echo "❌ Duplicate Autopilot winner detected"
  exit 1
fi


# 2) Compare locals vs CoW
join "$LOCAL" "$COW" | awk '
{
    local_addr=$2
    local_score=$3
    cow_addr=$4
    cow_score=$5

    if (local_addr != cow_addr || local_score != cow_score) {
        printf "❌ MISMATCH auction_id=%s\n", $1
        printf "    Autopilot:    addr=%s score=%s\n", cow_addr, cow_score
        printf "    Solver:       addr=%s score=%s\n", local_addr, local_score
        exit 1
    }
    else {
        printf "✅ MATCH auction_id=%s addr=%s score=%s\n", $1, cow_addr, cow_score
    }
}
'

# 3) Missing Autopilot for a solver
if join -v1 "$LOCAL" "$COW" | grep -q .; then
  echo "❌ Solver winner exists without Autopilot winner:"
  join -v1 "$LOCAL" "$COW"
  exit 1
fi

# 4) Autopilot without solvers
if join -v2 "$LOCAL" "$COW" | grep -q .; then
  echo "❌ Autopilot winner has no solver winner:"
  join -v2 "$LOCAL" "$COW"
  exit 1
fi


echo
echo "✅ All solver winners match Autopilot winner."

# Cleanup
rm -f "$COW" "$LOCAL" "$LOG"
