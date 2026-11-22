#!/bin/sh
# factory/probes/two-team.sh: a throwaway probe for the two-team PR loop (PO, 2026-11-21); delete after.
# Prints the mean of its integer arguments.
if [ "$#" -eq 0 ]; then
    echo 'usage: two-team.sh <n>...' >&2
    exit 2
fi

sum=0
for n in "$@"; do sum=$((sum + n)); done
echo $((sum / $#))
