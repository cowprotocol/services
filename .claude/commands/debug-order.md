---
description: Debug why a CoW Protocol order failed to match
---

Debug order: $ARGUMENTS

Read and follow the instructions in ./docs/COW_ORDER_DEBUG_SKILL.md to investigate this order.

Key steps:
1. Parse the order UID and network from arguments (default: mainnet)
2. Fetch order data from API to get status and details
3. Check order_events in DB for lifecycle events
4. Search Victoria Logs for the order UID
5. Identify root cause and report findings with evidence
6. If you haven't found anything go wil and try all SQL / log searches / codebase searches you can think of

Always show your evidence (log lines, DB results, API responses) when presenting findings.
