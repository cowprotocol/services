#!/bin/sh

# Check if heaptrack is enabled as the first argument
if [ "$1" = "heaptrack" ]; then
    # Remove 'heaptrack' from the arguments
    shift
    # Execute the remaining command with heaptrack
    exec heaptrack -o "/tmp/heaptrack/heaptrack.$(hostname).$(date +%s%N | cut -b1-13).gz" "$@"
else
    # Execute the command normally
    exec "$@"
fi
