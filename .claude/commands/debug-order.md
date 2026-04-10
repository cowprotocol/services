---
description: Debug why a CoW Protocol order failed to match
---

Debug order: $ARGUMENTS

Read and follow the instructions in ./docs/COW_ORDER_DEBUG_SKILL.md to investigate this order.

Key steps:
1. Parse the order UID and network from arguments (default: mainnet)
2. **Start with the debug endpoint** — fetch the comprehensive debug report first:
   ```bash
   source .env.claude && curl -s -H "X-API-Key: $COW_DEBUG_API_KEY" "https://partners.cow.fi/$NETWORK/api/v1/debug/order/$ORDER_UID" | jq .
   ```
   This returns order details, lifecycle events, auction participation, proposed solutions, executions, trades, and settlement attempts — all in one call.
3. Analyze the debug report — key event meanings:
   - `ready` = order made it into an auction (was sent to solvers)
   - `considered` = a solver included this order in a solution but that solution didn't win
   - `executing` = order is in the winning solution, being submitted on-chain
   - `traded` = order was settled on-chain
   - `filtered` / `invalid` = order was excluded (check the `reason` field)
4. Search Victoria Logs for additional context (filter reasons, error details, solver logs)
   - For finding discarded solutions where the order UID appears in calldata, use regex: `.*ORDER_UID_WITHOUT_0X.*` plus `discarded`
5. Use DB queries or API calls only if the debug report is missing info or you need deeper investigation
6. Identify root cause and report findings with evidence
7. If you haven't found anything go wild and try all SQL / log searches / codebase searches you can think of

Always show your evidence (log lines, DB results, API responses) when presenting findings.
