#!/bin/sh
# Runs heaptrack for the orderbook binary only
if [ "$1" = "orderbook" ]; then
  # Shift the 'orderbook' argument off the argument list
  shift
  # Prepend heaptrack to the command
  set -- heaptrack -o "/tmp/heaptrack/heaptrack.orderbook.$(shuf -i 1-99999 -n 1).gz" orderbook
fi

exec "$@"
