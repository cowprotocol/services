# Explorer Testing Guide

Complete testing documentation for the CoW Protocol Playground Block Explorer and Transaction Analysis implementation.

## Overview

This implementation includes a comprehensive local block explorer with advanced transaction analysis capabilities for the CoW Protocol Playground.

## What's Implemented

### Core Features (Milestone 1: Basic Explorer)
- Block retrieval (by number, hash, or "latest")
- Transaction details with receipts
- Call tree traces via debug_traceTransaction
- Universal search functionality
- Address summaries and transaction history

### Enhanced Features (Milestone 2: Enhanced UI)
- Latest blocks feed with auto-refresh (2s interval)
- Latest transactions feed with auto-refresh
- ABI decoding for common functions (ERC20, ERC721, Uniswap, etc.)
- Event and log decoding
- Background indexer syncing blocks/transactions to SQLite

### Verification (Milestone 3)
- Sourcify integration for contract verification
- ABI retrieval from local Sourcify repository
- Source code display for verified contracts
- Verification status checking
- Automatic ABI caching in SQLite

### Advanced Debugging (Milestone 4)
- Step-by-step transaction debugger with pagination
- Gas profiling by contract and function
- Source code mapping support
- Stack and memory inspection toggles
- Call depth tracking
- Detailed gas reports

---

## Quick Start

### Prerequisites
1. Docker Desktop running
2. Available ports: 8081 (API), 8083 (Web), 5555 (Sourcify), 8545 (RPC)
3. `.env` file configured (see Configuration section)

### Start Services

```bash
cd /Users/mitch/CoW-Playground/services/playground

# Local mode (no external RPC required)
docker compose -f docker-compose.local.yml up --build

# OR Fork mode (requires ETH_RPC_URL in .env)
docker compose -f docker-compose.fork.yml up --build
```

### Run Tests

```bash
# Run all tests (recommended)
./test.sh all

# Quick health check
./test.sh health

# Test specific components
./test.sh api           # API endpoints only
./test.sh ui            # Web UI only
./test.sh integration   # Docker integration

# Show help
./test.sh help
```

### Access Services

- Explorer Web UI: http://localhost:8083
- Explorer API: http://localhost:8081
- API Health Check: http://localhost:8081/healthz
- API Metrics: http://localhost:8081/metrics
- Sourcify: http://localhost:5555
- Chain RPC: http://localhost:8545

---

## Configuration

### Environment Variables

Create or edit `.env` in the `playground/` directory:

```bash
# PostgreSQL Configuration
POSTGRES_USER=postgres
POSTGRES_PASSWORD=postgres

# Chain Configuration
CHAIN=1
ENV=local

# Fork Mode (only needed for docker-compose.fork.yml)
ETH_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/YOUR_API_KEY

# Explorer Configuration (optional, has defaults)
INDEXER_INTERVAL_MS=1500    # How often to poll for new blocks
INDEXER_BATCH=50            # Blocks per indexing batch
HISTORY_LIMIT=10000         # Maximum blocks to keep in database
```

### Service Configuration

**Explorer API** (`explorer-api/`):
- JSON_RPC_URL: Ethereum node endpoint
- DB_PATH: SQLite database location
- NETWORK_NAME: Network identifier (e.g., "local", "mainnet")
- ENABLE_TRACE_STEPS: Enable step-by-step debugger
- CHAIN_ID: Chain ID (31337 for local, 1 for mainnet)

**Explorer Web** (`explorer-web/`):
- NEXT_PUBLIC_API_BASE: Explorer API base URL

---

## Testing

### Automated Test Suites

#### Test Suite

The consolidated test suite provides comprehensive coverage:

```bash
# Run all tests (~60+ test cases)
./test.sh all

# Or run specific test suites
./test.sh health        # Service health checks
./test.sh api           # API endpoints (blocks, transactions, traces, etc.)
./test.sh ui            # Web UI pages
./test.sh integration   # Docker integration checks
```

Test coverage includes:
- **Service Health**: All services running and accessible
- **Block API**: Retrieval by number, hash, "latest"
- **Transaction API**: Details, receipts, decoding
- **Trace API**: Call trees, step debugger
- **Debug API**: Gas reports, profiling
- **Search API**: Hash, address, block number detection
- **Address API**: Summaries, transaction history
- **Decode API**: Function and event decoding
- **Verification API**: Sourcify integration
- **Web UI**: All pages rendering correctly
- **Docker Integration**: Container and volume checks

### Manual Testing

#### Test Block Explorer
1. Open http://localhost:8083
2. Verify latest blocks and transactions feeds are updating
3. Search for a block number (e.g., "latest" or "100")
4. Search for a transaction hash
5. Search for an address
6. Verify all data displays correctly

#### Test Transaction Debugging
1. Navigate to any transaction page
2. Verify "Transaction Details" section shows all data
3. Check "Debug Information" section for:
   - Call tree visualization
   - Gas report with breakdown
   - Step-by-step debugger controls
4. Test debugger pagination (if transaction has many steps)
5. Toggle stack/memory inspection

#### Test Contract Verification
1. Deploy a contract and verify it via Sourcify
2. Navigate to the contract address page
3. Verify "Verification Status" shows as verified
4. Check ABI is displayed
5. Check source code is readable (if verified)

#### Test Frontend Integration
1. Open CoW Swap frontend at http://localhost:8000
2. Find any "View on Etherscan" link
3. Verify it redirects to http://localhost:8083 instead
4. Verify explorer shows transaction/address details

---

## API Endpoints

### Block Endpoints
```
GET /api/blocks/latest        # Latest block
GET /api/blocks/:id           # Block by number or hash
GET /api/blocks?limit=20      # Recent blocks list
```

### Transaction Endpoints
```
GET /api/tx/:hash             # Transaction details + receipt
GET /api/tx?limit=20          # Recent transactions list
GET /api/tx/:hash/trace?mode=tree   # Call tree visualization
GET /api/tx/:hash/trace?mode=steps  # Raw debug trace
GET /api/tx/:hash/steps?from=0&to=100&stack=1&memory=1  # Step debugger
GET /api/tx/:hash/gas-report  # Gas profiling report
```

### Address Endpoints
```
GET /api/address/:address     # Address summary
GET /api/address/:address/txs?limit=20  # Transaction history
```

### Search Endpoint
```
GET /api/search?q=<query>     # Auto-detect hash/address/block
```

### Decode Endpoints
```
POST /api/decode/function     # Decode function call
POST /api/decode/event        # Decode event log
```

### Verification Endpoints
```
GET /api/verify/status/:address  # Check verification status
GET /api/abi/:address            # Get contract ABI
GET /api/source/:address         # Get source code
```

### System Endpoints
```
GET /healthz                  # Health check
GET /metrics                  # Prometheus metrics
```

---

## Web UI Pages

```
/                             # Home: search + latest feeds
/block/:id                    # Block details
/tx/:hash                     # Transaction details + debugging
/address/:address             # Address summary + history
```

---

## Architecture

### Services

**explorer-api** (Fastify + TypeScript)
- REST API server on port 8081
- SQLite database with WAL mode
- Background indexer for blocks/transactions
- RPC proxy with caching
- Prometheus metrics

**explorer-web** (Next.js + React)
- Web interface on port 8083
- Server-side rendering
- SWR for auto-refresh feeds
- Responsive design

**sourcify** (Official Docker image)
- Contract verification service on port 5555
- Local repository storage
- API for verification checks

**chain** (Anvil)
- Local Ethereum node on port 8545
- Fork or local mode
- Debug trace support

### Data Flow

```
User Request
    ↓
Explorer Web UI (Next.js)
    ↓
Explorer API (Fastify)
    ↓
┌──────────┬──────────┬──────────┐
↓          ↓          ↓          ↓
SQLite   RPC Node  Sourcify  Signatures
(cache)   (chain)  (verify)  (decode)
```

### Caching Strategy

1. **SQLite Database**: Blocks, transactions, receipts cached
2. **ABI Cache**: Verified contract ABIs stored locally
3. **Background Indexer**: Continuous syncing from RPC
4. **History Pruning**: Automatic cleanup of old data (configurable limit)

---

## Troubleshooting

### Services Won't Start

**Docker daemon not running**
```bash
# Check Docker status
docker ps

# Start Docker Desktop
# macOS: Open Docker Desktop app
# Linux: sudo systemctl start docker
```

**Port conflicts**
```bash
# Check what's using ports
lsof -i :8081
lsof -i :8083

# Stop conflicting services or change ports in docker-compose
```

**RPC authentication errors (fork mode)**
```bash
# Update .env with valid RPC URL
ETH_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/YOUR_VALID_KEY

# Or switch to local mode
docker compose -f docker-compose.local.yml up
```

### Tests Failing

**Services not ready**
```bash
# Wait for all services to be healthy
docker ps

# Check logs
docker logs playground-explorer-api-1
docker logs playground-chain-1

# Give services 30 seconds to initialize before running tests
```

**No transactions available**
```bash
# Send a test transaction first
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "method":"eth_sendTransaction",
    "params":[{
      "from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      "to":"0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      "value":"0xde0b6b3a7640000"
    }],
    "id":1
  }'
```

### Explorer API Issues

**Database locked errors**
```bash
# SQLite WAL mode handles this, but if issues persist:
docker compose down -v  # Clear volumes
docker compose up --build
```

**Indexer not syncing**
```bash
# Check indexer logs
docker logs playground-explorer-api-1 | grep -i index

# Verify RPC is accessible
curl http://localhost:8545 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

**Sourcify verification not working**
```bash
# Check Sourcify is running
curl http://localhost:5555/health

# Restart Sourcify if needed
docker compose restart sourcify
```

### Web UI Issues

**Feeds not updating**
- Check browser console for errors
- Verify API is accessible: http://localhost:8081/healthz
- Clear browser cache

**Pages not loading**
- Check explorer-web logs: `docker logs playground-explorer-web-1`
- Verify Next.js built successfully
- Check NEXT_PUBLIC_API_BASE environment variable

---

## Performance

### Expected Response Times

- Block retrieval: < 500ms
- Transaction details: < 1s (with decoding)
- Latest feeds: < 100ms (from cache)
- Call traces: < 2s (on-demand from RPC)
- Gas reports: < 3s (complex analysis)
- Step debugger: < 1s per page (200 steps)

### Optimization

**Background Indexer Configuration**
```bash
# Faster syncing (more RPC load)
INDEXER_INTERVAL_MS=500
INDEXER_BATCH=100

# Slower syncing (less RPC load)
INDEXER_INTERVAL_MS=3000
INDEXER_BATCH=20
```

**History Management**
```bash
# Keep more history (uses more disk space)
HISTORY_LIMIT=50000

# Keep less history (saves disk space)
HISTORY_LIMIT=5000
```

### Monitoring

Access Prometheus metrics at http://localhost:8081/metrics

Key metrics:
- `explorer_blocks_indexed_total`: Total blocks indexed
- `explorer_transactions_indexed_total`: Total transactions indexed
- `explorer_api_request_duration_seconds`: API response times
- `explorer_rpc_requests_total`: RPC call count
- `explorer_cache_hits_total`: Cache hit rate

---

## Development

### Project Structure

```
playground/
├── explorer-api/              # Backend API
│   ├── src/
│   │   ├── server.ts         # Main API server
│   │   ├── db.ts             # SQLite database
│   │   ├── rpc.ts            # RPC client
│   │   ├── services.ts       # Business logic
│   │   ├── verify.ts         # Sourcify integration
│   │   ├── signatures.ts     # Function/event signatures
│   │   └── sourceMap.ts      # Source code mapping
│   ├── package.json
│   └── Dockerfile
│
├── explorer-web/              # Frontend UI
│   ├── pages/
│   │   ├── index.tsx         # Home page
│   │   ├── block/[id].tsx    # Block page
│   │   ├── tx/[hash].tsx     # Transaction page
│   │   └── address/[address].tsx  # Address page
│   ├── package.json
│   └── Dockerfile
│
├── docker-compose.fork.yml    # Fork mode configuration
├── docker-compose.local.yml   # Local mode configuration
├── test.sh                    # Consolidated test suite
└── TESTING_GUIDE.md          # This file
```

### Making Changes

**Explorer API**
```bash
# API code is in explorer-api/src/
# Changes auto-reload with Docker volumes

# Run locally for development
cd explorer-api
npm install
npm run dev
```

**Explorer Web**
```bash
# Web code is in explorer-web/pages/
# Changes auto-reload in development mode

# Run locally for development
cd explorer-web
npm install
npm run dev
```

### Adding Tests

Add test cases to `test.sh` by editing the appropriate test function:

```bash
# Add to test_api() function
test_endpoint "My new endpoint" "$EXPLORER_API/api/my-endpoint" "expected_pattern"

# Or for custom tests, add a new function
test_my_feature() {
    echo -e "\n${BLUE}=== My Feature Tests ===${NC}\n"

    test_endpoint "Feature X" "$EXPLORER_API/api/feature-x" "success"
    # Add more tests...
}

# Then call it in main()
```

---

## Support

For issues or questions:

1. Check this testing guide
2. Review test script output for specific errors
3. Check Docker logs: `docker logs <container-name>`
4. Review RFP_COMPLIANCE.md for feature documentation
5. Check README.md for general usage

---

## Production Deployment

### Checklist

- [ ] Configure appropriate HISTORY_LIMIT for disk space
- [ ] Set up monitoring alerts for Prometheus metrics
- [ ] Configure backup strategy for SQLite database
- [ ] Review and adjust INDEXER_INTERVAL_MS for load
- [ ] Set up log aggregation (if needed)
- [ ] Configure firewall rules for ports
- [ ] Set up SSL/TLS if exposing publicly
- [ ] Document recovery procedures

### Security Considerations

- Explorer API has no authentication (designed for local/private use)
- Do not expose publicly without adding authentication
- RPC endpoint should be trusted
- Sourcify repository contains user-submitted code (review before use)

---

## Related Documentation

- **RFP_COMPLIANCE.md** - Complete RFP compliance report
- **README.md** - Main playground documentation
- **PR_CHECKLIST.md** - PR submission guide

---

Last Updated: October 24, 2025
