#!/bin/sh
# Runs heaptrack for the orderbook binary only
if [ "$1" = "orderbook" ]; then
  exec /bin/sh -c "heaptrack -o /tmp/heaptrack/heaptrack.orderbook.$(shuf -i 1-99999 -n 1).gz /usr/local/bin/orderbook"
else
  exec "$@"
fi
