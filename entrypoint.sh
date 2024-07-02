#!/bin/sh
if [ "$1" = "orderbook" ]; then
  # Shift the 'orderbook' argument off the argument list
  shift
  # Run heaptrack to profile the 'orderbook' process
  # Redirect the output to a file in /tmp/heaptrack with a unique name based on a random number
  exec heaptrack -o "/tmp/heaptrack/heaptrack.orderbook.$(shuf -i 1-99999 -n 1).gz" /usr/local/bin/orderbook "$@"
else
  exec "$@"
fi
