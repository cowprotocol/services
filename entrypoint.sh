#!/bin/sh

# Default to "unknown" if POD_NAME is not set
pod_name=${POD_NAME:-unknown}

# Check if heaptrack is enabled as the first argument
if [ "$1" = "heaptrack" ]; then
    # Remove 'heaptrack' from the arguments
    shift
    # Execute the remaining command with heaptrack
    exec heaptrack -o "/tmp/heaptrack/heaptrack.$pod_name.$(shuf -i 1-99999 -n 1).gz" "$@"
else
    # Execute the command normally
    exec "$@"
fi
