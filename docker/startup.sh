#!/bin/sh
# The purpose of this script is to set a log regex to forward error logs to stderr and other logs to
# stdout. This allows us to easily configure alerts when any message goes to stderr.
"$@" | regex-stream-split "^\d+-\d+-\d+T\d+:\d+:\d+\.\d+Z\s+(TRACE|DEBUG|INFO|WARN)" "^\d+-\d+-\d+T\d+:\d+:\d+\.\d+Z\s+ERROR"
