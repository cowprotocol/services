# Delta Sync Headers

This document describes the HTTP headers used by the delta sync and solve request flows.

## Delta Sync Authentication

- X-Delta-Sync-Api-Key
  - Required when AUTOPILOT_DELTA_SYNC_API_KEY is set on the autopilot.
  - Value: opaque API key string.

## Solve Request Metadata Headers

These headers are used by the driver to parse thin solve requests and to reconstruct
solve requests from the delta replica.

- X-Auction-Id
  - Type: integer
  - Example: 42

- X-Auction-Deadline
  - Type: RFC3339 timestamp
  - Example: 2025-01-02T03:04:05Z

- X-Auction-Body-Mode
  - Type: string
  - Values: full | thin

- X-Auction-Tokens
  - Type: comma-separated list of address/trusted pairs
  - Format: <address>:<trusted>,<address>:<trusted>
  - address: 0x-prefixed 20-byte hex
  - trusted: 1 | 0 | true | false (case-insensitive for TRUE/FALSE)
  - Example: 0x0000000000000000000000000000000000000001:true,0x0000000000000000000000000000000000000002:false

- X-Auction-Jit-Order-Owners
  - Type: comma-separated list of addresses
  - Format: <address>,<address>
  - Example: 0x0000000000000000000000000000000000000003,0x0000000000000000000000000000000000000004

## Delta Event Versioning

Delta envelopes include a protocol version and may contain event types that
older drivers do not understand. Unknown events are ignored for forward
compatibility. Breaking changes require a protocol version bump and explicit
client/server coordination.
