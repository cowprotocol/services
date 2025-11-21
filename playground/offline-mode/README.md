# CoW Protocol Offline Playground

A self-contained, offline development environment for CoW Protocol that runs locally without requiring mainnet forks or archive nodes.

## What is this?

The CoW Protocol Offline Playground is a complete local blockchain environment that includes:

- **Local Anvil node** with persistent state
- **CoW Protocol contracts**: Settlement, VaultRelayer, Authenticator
- **DEX infrastructure**: Uniswap V2 (with liquidity pools)
- **Test tokens**: WETH, USDC, DAI
- **CoW Protocol services**: Orderbook API, Autopilot, Driver, Baseline Solver
- **Mock Balancer Vault** for settlement execution

All services work out-of-the-box with proper configuration pointing to the local blockchain.

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Foundry (forge, cast, anvil)
- jq (for JSON parsing)
- Python 3 (for order signing)

### Initialize the Environment

1. **Start all services** (this will automatically load the existing blockchain state):
   ```bash
   cd /path/to/playground
   docker-compose -f docker-compose.offline.yml up -d
   ```

   The Anvil node will automatically load the pre-deployed state from `poc-offline-mode/state/poc-state.json`.

2. **Wait for services to be ready**:
   ```bash
   # Wait for orderbook API to be available
   curl --retry 24 --retry-delay 5 --retry-all-errors http://localhost:8080/api/v1/version
   ```

3. **Run the end-to-end test**:
   ```bash
   ./test_playground_offline_cow.sh
   ```

   This script will:
   - Create two orders (peer-to-peer matching)
   - Wait for autopilot to match and settle them
   - Verify balances changed correctly

### Access Points

Once running, you can access:

- **Orderbook API**: http://localhost:8080
- **Anvil RPC**: http://localhost:8545
- **Driver API**: http://localhost:9000
- **Grafana (monitoring)**: http://localhost:3000
- **Prometheus (metrics)**: http://localhost:9090
- **Adminer (database)**: http://localhost:8082

### Contract Addresses

All deployed contract addresses are stored in:
```
poc-offline-mode/config/addresses.json
```

Example:
```json
{
  "chainId": "31337",
  "tokens": {
    "WETH": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
    "USDC": "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
    "DAI": "0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9"
  },
  "cowProtocol": {
    "settlement": "0xB7f8BC63BbcaD18155201308C8f3540b07f84F5e",
    "vaultRelayer": "0x8dAF17A20c9DBA35f005b6324F493785D239719d",
    "balancerVault": "0x8A791620dd6260079BF849Dc5567aDC3F2FdC318"
  }
}
```

## Building Contracts from Source

If you need to rebuild contracts (for example, after modifying sources), use the Foundry profiles:

### Profile Overview

The project uses three Foundry profiles to handle different Solidity versions:

| Profile | Solidity Version | Contracts | Output Directory |
|---------|------------------|-----------|------------------|
| `default` | 0.8.26 | Custom contracts (tokens, mocks) | `contracts/out` |
| `uniswap-v2` | 0.5.16 | Uniswap V2 Core (Factory, Pair) | `contracts/out-uniswap-v2` |
| `uniswap-v2-periphery` | 0.6.6 | Uniswap V2 Router | `contracts/out-uniswap-v2-periphery` |
| `cow-protocol` | 0.7.6 | CoW Protocol contracts | `contracts/out-cow-protocol` |

### Building Specific Contracts

From the `poc-offline-mode` directory:

```bash
# Build custom contracts (default profile)
forge build

# Build Uniswap V2 Core (Factory, Pair)
forge build --profile uniswap-v2

# Build Uniswap V2 Router
forge build --profile uniswap-v2-periphery

# Build CoW Protocol contracts
forge build --profile cow-protocol
```

### Build All Contracts

To rebuild everything:

```bash
forge build && \
forge build --profile uniswap-v2 && \
forge build --profile uniswap-v2-periphery && \
forge build --profile cow-protocol
```

## Deploying from Scratch

If you want to deploy everything from scratch (instead of loading the existing state):

1. **Delete the existing state**:
   ```bash
   rm poc-offline-mode/state/poc-state.json
   ```

2. **Start Anvil and services**:
   ```bash
   docker-compose -f docker-compose.offline.yml up -d chain
   ```

3. **Run the deployment script**:
   ```bash
   cd poc-offline-mode
   ./scripts/deploy-all.sh
   ```

   This will deploy:
   - Step 1: Tokens (WETH, USDC, DAI)
   - Step 2: Uniswap V2 (Factory, Router)
   - Step 3: Mock Balancer Vault
   - Step 4: CoW Protocol (Settlement, VaultRelayer, Authenticator)
   - Step 5: Uniswap V2 Pairs with liquidity
   - Step 6: Save addresses to `config/addresses.json`

4. **Start the remaining services**:
   ```bash
   docker-compose -f docker-compose.offline.yml up -d
   ```

### Important Note: UniswapV2Library Init Code Hash

⚠️ **Known Issue**: If you deploy from scratch, you may need to update the init code hash in UniswapV2Library.sol

The Uniswap V2 Router uses a hardcoded init code hash to compute pair addresses via CREATE2. The hash differs between Foundry and Hardhat/Truffle deployments.

**Mainnet hash** (original):
```solidity
hex'96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f'
```

**Foundry hash** (for local deployment):
```solidity
hex'b6912aa8f91da604bdd903b3484a9f6bb569baa993085fc590133487ff27f91e'
```

**Location**: `contracts/lib/v2-periphery/contracts/libraries/UniswapV2Library.sol` (line 24)

**How to fix**:
```solidity
// calculates the CREATE2 address for a pair without making any external calls
function pairFor(address factory, address tokenA, address tokenB) internal pure returns (address pair) {
    (address token0, address token1) = sortTokens(tokenA, tokenB);
    pair = address(uint(keccak256(abi.encodePacked(
            hex'ff',
            factory,
            keccak256(abi.encodePacked(token0, token1)),
            hex'b6912aa8f91da604bdd903b3484a9f6bb569baa993085fc590133487ff27f91e' // Foundry init code hash
        ))));
}
```

After updating, rebuild the periphery contracts:
```bash
forge build --profile uniswap-v2-periphery
```

**Note**: The current `poc-state.json` already has the correct hash deployed, so this is only needed if deploying from scratch.

## Configuration Files

### Docker Configuration

- **`docker-compose.offline.yml`**: Defines all services (chain, database, orderbook, autopilot, driver, baseline solver)
- **`.env.offline`**: Environment variables for services

### Solver Configuration

- **`configs/offline/driver.toml`**: Driver configuration for offline mode
  - Chain ID: 31337
  - Settlement contract address
  - Gas estimation settings

### Blockchain State

- **`state/poc-state.json`**: Persistent Anvil blockchain state
  - Contains all deployed contracts
  - Pre-seeded liquidity pools
  - Can be loaded/dumped by Anvil

## Architecture

```
┌─────────────────┐
│  Anvil (31337)  │  ← Local blockchain with persistent state
└────────┬────────┘
         │
    ┌────┴─────────────────────────────────┐
    │                                       │
┌───▼────────┐                    ┌────────▼──────┐
│   Tokens   │                    │  DEX Contracts │
│ WETH, USDC │                    │   Uniswap V2   │
│    DAI     │                    │  (with pools)  │
└────────────┘                    └────────────────┘
                                           │
         ┌─────────────────────────────────┤
         │                                 │
    ┌────▼──────────┐          ┌──────────▼──────┐
    │  CoW Protocol │          │ Balancer Vault  │
    │   Settlement  │◄─────────│     (Mock)      │
    │ VaultRelayer  │          └─────────────────┘
    └───────┬───────┘
            │
    ┌───────┴──────────────────────────────┐
    │                                       │
┌───▼─────┐  ┌──────────┐  ┌──────┐  ┌────▼────┐
│Orderbook│  │Autopilot │  │Driver│  │Baseline │
│   API   │  │          │  │      │  │ Solver  │
└─────────┘  └──────────┘  └──────┘  └─────────┘
```

## Troubleshooting

### Services not starting

Check Docker logs:
```bash
docker-compose -f docker-compose.offline.yml logs -f [service_name]
```

Services: `chain`, `orderbook`, `autopilot`, `driver`, `baseline`

### Orders not settling

1. Check if services are running:
   ```bash
   docker-compose -f docker-compose.offline.yml ps
   ```

2. Check driver logs for errors:
   ```bash
   docker-compose -f docker-compose.offline.yml logs driver --tail=50
   ```

3. Verify token approvals are set for VaultRelayer

### Reset everything

```bash
# Stop all services
docker-compose -f docker-compose.offline.yml down -v

# Remove state (optional - will require redeployment)
rm poc-offline-mode/state/poc-state.json

# Start fresh
docker-compose -f docker-compose.offline.yml up -d
```

## Development Workflow

1. **Make code changes** to contracts or services
2. **Rebuild contracts** using appropriate Foundry profile
3. **Redeploy** using `scripts/deploy-all.sh` (or keep existing state)
4. **Restart services**: `docker-compose -f docker-compose.offline.yml restart`
5. **Test changes** using `test_playground_offline_cow.sh`

## Learn More

- [CoW Protocol Documentation](https://docs.cow.fi/)
- [Foundry Book](https://book.getfoundry.sh/)
- [Grant Application](grant_application-by-hand.md) - Full project roadmap and architecture

## License

This project is part of the CoW Protocol ecosystem and follows the same open-source licensing.
