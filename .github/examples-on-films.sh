#!/bin/sh
# Runs every example on every film in $1 (one folder per film) and fails when
# one exits with an error or prints nothing. The examples must be built:
# cargo build --release --all-features --examples.
set -u
films=${1:?usage: examples-on-films.sh <films folder>}
bin=target/release/examples
failed=0
count=0
for dir in "$films"/*/; do
    count=$((count + 1))
    # The first player id the players example lists, for the timeline.
    id=$("$bin/players" "$dir" | awk '$1 ~ /^[0-9]+$/ { print $1; exit }')
    for run in "players" "kills" "raw" "trails" "json" "timeline ${id:-0}" "feed ${id:-0}" "playback 60"; do
        set -- $run
        name=$1
        shift
        if ! out=$("$bin/$name" "$dir" "$@" 2>&1) || [ -z "$out" ]; then
            echo "$name failed on $dir: $(printf '%s' "$out" | tail -n 1)"
            failed=$((failed + 1))
        fi
    done
done
echo "$count films, $failed failures"
[ "$failed" -eq 0 ]
