#!/bin/bash
set -e

echo "🚀 Deploying all contracts to local Anvil..."
echo ""

# Set defaults if not provided (don't source .env in Docker - it overrides env vars)
: ${RPC_URL:=http://localhost:8545}
: ${DEPLOYER_PRIVATE_KEY:=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}

echo "Using RPC URL: $RPC_URL"
echo ""

# Create directories
mkdir -p config
mkdir -p state

# Step 1: Deploy Tokens
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 1: Deploying Tokens (WETH, USDC, DAI)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
forge script contracts/script/DeployTokens.s.sol:DeployTokens \
    --rpc-url $RPC_URL \
    --broadcast \
    --private-key $DEPLOYER_PRIVATE_KEY \
    --skip-simulation \
    -vvv

echo ""
echo "✅ Tokens deployed!"
echo ""

# Extract token addresses from broadcast - using array indexing for TestERC20
WETH_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "WETH" and .transactionType == "CREATE") | .contractAddress' broadcast/DeployTokens.s.sol/31337/run-latest.json)
USDC_ADDRESS=$(jq -r '[.transactions[] | select(.contractName == "TestERC20" and .transactionType == "CREATE")] | .[0].contractAddress' broadcast/DeployTokens.s.sol/31337/run-latest.json)
DAI_ADDRESS=$(jq -r '[.transactions[] | select(.contractName == "TestERC20" and .transactionType == "CREATE")] | .[1].contractAddress' broadcast/DeployTokens.s.sol/31337/run-latest.json)

# Export for next scripts
export WETH_ADDRESS
export USDC_ADDRESS
export DAI_ADDRESS

echo "📝 Deployed token addresses:"
echo "  WETH: $WETH_ADDRESS"
echo "  USDC: $USDC_ADDRESS"
echo "  DAI: $DAI_ADDRESS"
echo ""

# Step 2: Deploy Uniswap V2
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 2: Deploying Uniswap V2 (Factory + Router + Pools)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
forge script contracts/script/DeployUniswapV2.s.sol:DeployUniswapV2 \
    --rpc-url $RPC_URL \
    --broadcast \
    --private-key $DEPLOYER_PRIVATE_KEY \
    --skip-simulation \
    -vvv

echo ""
echo "✅ Uniswap V2 deployed!"
echo ""

# Extract Uniswap addresses from broadcast - deployed via deployCode so no contractName
UNISWAP_FACTORY=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[0].contractAddress' broadcast/DeployUniswapV2.s.sol/31337/run-latest.json)
UNISWAP_ROUTER=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[1].contractAddress' broadcast/DeployUniswapV2.s.sol/31337/run-latest.json)
# Get pair addresses from PairCreated event logs
PAIR_WETH_USDC=$(jq -r '.receipts[] | select(.transactionHash != null and .logs != null) | .logs[] | select(.topics[0] == "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9") | .address' broadcast/DeployUniswapV2.s.sol/31337/run-latest.json | sed -n '1p')
PAIR_WETH_DAI=$(jq -r '.receipts[] | select(.transactionHash != null and .logs != null) | .logs[] | select(.topics[0] == "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9") | .address' broadcast/DeployUniswapV2.s.sol/31337/run-latest.json | sed -n '2p')
PAIR_USDC_DAI=$(jq -r '.receipts[] | select(.transactionHash != null and .logs != null) | .logs[] | select(.topics[0] == "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9") | .address' broadcast/DeployUniswapV2.s.sol/31337/run-latest.json | sed -n '3p')

# Export for next scripts
export UNISWAP_FACTORY
export UNISWAP_ROUTER
export PAIR_WETH_USDC
export PAIR_WETH_DAI
export PAIR_USDC_DAI

echo "📝 Deployed Uniswap addresses:"
echo "  Factory: $UNISWAP_FACTORY"
echo "  Router: $UNISWAP_ROUTER"
echo "  WETH-USDC Pair: $PAIR_WETH_USDC"
echo "  WETH-DAI Pair: $PAIR_WETH_DAI"
echo "  USDC-DAI Pair: $PAIR_USDC_DAI"
echo ""

# Step 3: Deploy CoW Protocol
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 3: Deploying CoW Protocol (Settlement + Auth)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
forge script contracts/script/DeployCowProtocol.s.sol:DeployCowProtocol \
    --rpc-url $RPC_URL \
    --broadcast \
    --private-key $DEPLOYER_PRIVATE_KEY \
    --skip-simulation \
    -vvv

echo ""
echo "✅ CoW Protocol deployed!"
echo ""

# Extract CoW Protocol addresses from broadcast - also deployed via deployCode
COW_AUTHENTICATOR=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[0].contractAddress' broadcast/DeployCowProtocol.s.sol/31337/run-latest.json)
COW_SETTLEMENT=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[1].contractAddress' broadcast/DeployCowProtocol.s.sol/31337/run-latest.json)
# VaultRelayer is created by Settlement contract, need to read it from chain
# cast call returns bytes32, we need to extract the address (last 20 bytes)
COW_VAULT_RELAYER=$(cast call $COW_SETTLEMENT "vaultRelayer()" --rpc-url $RPC_URL | xargs | sed 's/^0x0*//; s/^/0x/')
BALANCER_VAULT="0xBA12222222228d8Ba445958a75a0704d566BF2C8"

# Export for next scripts
export COW_AUTHENTICATOR
export COW_SETTLEMENT
export COW_VAULT_RELAYER
export BALANCER_VAULT

echo "📝 Deployed CoW Protocol addresses:"
echo "  Authenticator: $COW_AUTHENTICATOR"
echo "  Settlement: $COW_SETTLEMENT"
echo "  Vault Relayer: $COW_VAULT_RELAYER"
echo "  Balancer Vault: $BALANCER_VAULT"
echo ""

# Step 4: Add Liquidity
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 4: Adding Initial Liquidity"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "⏳ Coming soon..."
echo ""

# Step 5: Export Addresses to JSON
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 5: Exporting Deployed Addresses to JSON"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Source the addresses to make them available as environment variables
if [ -f config/addresses.txt ]; then
    source config/addresses.txt
fi

forge script contracts/script/ExportAddresses.s.sol:ExportAddresses \
    --rpc-url $RPC_URL \
    -vv

echo ""
echo "✅ Addresses exported to config/addresses.json"
echo ""

# Step 6: Generate Configuration Files
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 6: Generating Configuration Files"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Normalize address variable names for the config generator
export UNISWAP_V2_FACTORY_ADDRESS=${UNISWAP_FACTORY}
export UNISWAP_V2_ROUTER_ADDRESS=${UNISWAP_ROUTER}
export SETTLEMENT_CONTRACT_ADDRESS=${COW_SETTLEMENT}
export AUTHENTICATOR_ADDRESS=${COW_AUTHENTICATOR}
export VAULT_RELAYER_ADDRESS=${COW_VAULT_RELAYER}
export BALANCER_VAULT_ADDRESS=${BALANCER_VAULT}

forge script contracts/script/GenerateConfigs.s.sol:GenerateConfigs \
    --rpc-url $RPC_URL \
    -vv

echo ""
echo "✅ Configuration files generated!"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ DEPLOYMENT COMPLETE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📋 Deployment Summary:"
echo "  ✅ Step 1: Tokens deployed (WETH, USDC, DAI)"
echo "  ✅ Step 2: Uniswap V2 deployed (Factory, Router, Pairs)"
echo "  ✅ Step 3: CoW Protocol deployed (Settlement, Auth, VaultRelayer)"
echo "  ⏳ Step 4: Add liquidity (TODO)"
echo "  ✅ Step 5: Addresses exported to JSON"
echo "  ✅ Step 6: Configuration files generated"
echo ""
echo "📁 Output files:"
echo "  - config/addresses.json (deployment addresses)"
echo "  - configs/offline/driver.toml (auto-generated)"
echo "  - configs/offline/baseline.toml (auto-generated)"
echo "  - playground/.env.offline (auto-generated)"
echo "  - state/poc-state.json (blockchain state)"
echo ""
echo "🚀 Next: Start the full stack with:"
echo "  cd ../../playground"
echo "  docker compose -f docker-compose.offline.yml up"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
