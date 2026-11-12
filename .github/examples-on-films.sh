#!/bin/sh
# Runs every example on every film in $1 (one folder per film) and fails when
# one exits with an error or prints nothing. Films run $2 at a time (4 when
# not given). The examples must be built:
# cargo build --release --all-features --examples.
set -u
bin=target/release/examples

# One film: every example on it, a line for each that fails.
if [ "${1:-}" = "--film" ]; then
    dir=$2
    # The first player id the players example lists, for the timeline and
    # the feed.
    id=$("$bin/players" "$dir" | awk '$1 ~ /^[0-9]+$/ { print $1; exit }')
    for run in "players" "kills" "raw" "trails" "json" "timeline ${id:-0}" "feed ${id:-0}" "playback 60" "frame 60"; do
        set -- $run
        name=$1
        shift
        if ! out=$("$bin/$name" "$dir" "$@" 2>&1) || [ -z "$out" ]; then
            echo "FAILED $name on $dir: $(printf '%s' "$out" | tail -n 1)"
        fi
    done
    exit 0
fi

films=${1:?usage: examples-on-films.sh <films folder> [jobs]}
jobs=${2:-4}
count=$(find "$films" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
out=$(find "$films" -mindepth 1 -maxdepth 1 -type d | sort | xargs -P "$jobs" -I{} sh "$0" --film {})
failed=$(printf '%s\n' "$out" | grep -c '^FAILED')
printf '%s\n' "$out" | grep '^FAILED'
echo "$count films, $failed failures"
[ "$failed" -eq 0 ]
