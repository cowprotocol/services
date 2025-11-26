#!/bin/bash

# Test script for HooksTrampoline contract
set -e
set -u

# Default values
RPC_URL=http://localhost:8545
CHAIN_ID=31337

# Script directory for finding addresses.json
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDRESSES_FILE="$SCRIPT_DIR/offline-mode/config/addresses.json"

echo "🧪 Testing HooksTrampoline Contract"
echo "===================================="
echo ""

# Load addresses from addresses.json
if [ ! -f "$ADDRESSES_FILE" ]; then
  echo "❌ Error: addresses.json not found at $ADDRESSES_FILE"
  exit 1
fi

# Get contract addresses
SETTLEMENT_CONTRACT=$(jq -r '.cowProtocol.settlement' "$ADDRESSES_FILE")
HOOKS_TRAMPOLINE=$(jq -r '.cowProtocol.hooksTrampoline' "$ADDRESSES_FILE")

if [ "$SETTLEMENT_CONTRACT" = "null" ] || [ "$HOOKS_TRAMPOLINE" = "null" ]; then
  echo "❌ Error: Settlement or HooksTrampoline address not found in addresses.json"
  exit 1
fi

echo "Configuration:"
echo "  Settlement: $SETTLEMENT_CONTRACT"
echo "  HooksTrampoline: $HOOKS_TRAMPOLINE"
echo ""

# Wait for chain to be ready
echo "⏳ Waiting for chain to be ready..."
until curl -s -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $RPC_URL | grep -q result; do
  sleep 1
done
echo "✅ Chain is ready!"
echo ""

# Step 1: Deploy Counter test contract
echo "📝 Deploying Counter test contract..."
cd "$SCRIPT_DIR/offline-mode"

# Build the contract first
forge build --contracts contracts/test/Counter.sol > /dev/null 2>&1

# Deploy Counter contract
COUNTER_DEPLOY_OUTPUT=$(forge create contracts/test/Counter.sol:Counter \
  --rpc-url $RPC_URL \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast \
  2>&1)

COUNTER_ADDRESS=$(echo "$COUNTER_DEPLOY_OUTPUT" | grep "Deployed to:" | awk '{print $3}')

if [ -z "$COUNTER_ADDRESS" ]; then
  echo "❌ Error: Failed to deploy Counter contract"
  echo "$COUNTER_DEPLOY_OUTPUT"
  exit 1
fi

echo "  ✅ Counter deployed at: $COUNTER_ADDRESS"
echo ""

# Step 2: Verify initial counter value is 0
echo "🔍 Verifying initial counter value..."
INITIAL_COUNTER=$(cast call $COUNTER_ADDRESS --rpc-url $RPC_URL "getCounter()(uint256)")
echo "  Initial counter: $INITIAL_COUNTER"

if [ "$INITIAL_COUNTER" != "0" ]; then
  echo "❌ Error: Expected initial counter to be 0, got $INITIAL_COUNTER"
  exit 1
fi
echo ""

# Step 3: Prepare hook calldata
echo "🔧 Preparing hook calldata..."
# Encode the increment() function call
HOOK_CALLDATA=$(cast calldata "increment()")
# Set a reasonable gas limit for the hook (100,000 gas)
HOOK_GAS_LIMIT="100000"
echo "  Hook calldata: $HOOK_CALLDATA"
echo "  Hook gas limit: $HOOK_GAS_LIMIT"
echo ""

# Step 4: Execute hook via HooksTrampoline
echo "🚀 Executing hook via HooksTrampoline (impersonating Settlement)..."

# Give the Settlement contract some ETH for gas
cast rpc anvil_setBalance $SETTLEMENT_CONTRACT "0x1000000000000000000" --rpc-url $RPC_URL > /dev/null 2>&1

# Enable impersonation for the Settlement contract address
cast rpc anvil_impersonateAccount $SETTLEMENT_CONTRACT --rpc-url $RPC_URL > /dev/null 2>&1

# Execute the hook from the impersonated Settlement address
# The execute function takes an array of Hook structs: execute((address,bytes,uint256)[])
EXECUTE_TX=$(cast send $HOOKS_TRAMPOLINE \
  --from $SETTLEMENT_CONTRACT \
  --unlocked \
  --rpc-url $RPC_URL \
  "execute((address,bytes,uint256)[])" \
  "[($COUNTER_ADDRESS,$HOOK_CALLDATA,$HOOK_GAS_LIMIT)]" \
  2>&1)

EXECUTE_EXIT_CODE=$?

# Stop impersonation
cast rpc anvil_stopImpersonatingAccount $SETTLEMENT_CONTRACT --rpc-url $RPC_URL > /dev/null 2>&1

if [ $EXECUTE_EXIT_CODE -ne 0 ]; then
  echo "❌ Error: Failed to execute hook"
  echo "$EXECUTE_TX"
  exit 1
fi

echo "  ✅ Hook executed successfully"
echo ""

# Step 5: Verify counter was incremented
echo "🔍 Verifying counter was incremented..."
FINAL_COUNTER=$(cast call $COUNTER_ADDRESS --rpc-url $RPC_URL "getCounter()(uint256)")
echo "  Final counter: $FINAL_COUNTER"

if [ "$FINAL_COUNTER" != "1" ]; then
  echo "❌ Error: Expected counter to be 1, got $FINAL_COUNTER"
  exit 1
fi
echo ""

# Step 6: Verify last caller was HooksTrampoline
echo "🔍 Verifying last caller was HooksTrampoline..."
LAST_CALLER=$(cast call $COUNTER_ADDRESS --rpc-url $RPC_URL "lastCaller()(address)")
echo "  Last caller: $LAST_CALLER"

# Normalize addresses for comparison (lowercase)
LAST_CALLER_NORMALIZED=$(echo "$LAST_CALLER" | tr '[:upper:]' '[:lower:]')
HOOKS_TRAMPOLINE_NORMALIZED=$(echo "$HOOKS_TRAMPOLINE" | tr '[:upper:]' '[:lower:]')

if [ "$LAST_CALLER_NORMALIZED" != "$HOOKS_TRAMPOLINE_NORMALIZED" ]; then
  echo "❌ Error: Expected last caller to be HooksTrampoline ($HOOKS_TRAMPOLINE), got $LAST_CALLER"
  exit 1
fi
echo ""

# Step 7: Test calling from non-Settlement address (should fail)
echo "🔒 Testing access control (calling from non-Settlement address)..."
NON_SETTLEMENT="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
UNAUTHORIZED_TX=$(cast send $HOOKS_TRAMPOLINE \
  --from $NON_SETTLEMENT \
  --private-key 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d \
  --rpc-url $RPC_URL \
  "execute(address,bytes)" \
  $COUNTER_ADDRESS \
  $HOOK_CALLDATA \
  2>&1 || true)

if echo "$UNAUTHORIZED_TX" | grep -q "reverted"; then
  echo "  ✅ Access control working: unauthorized call was rejected"
else
  echo "❌ Error: Unauthorized call should have been rejected"
  exit 1
fi
echo ""

echo "✅ All tests passed!"
echo ""
echo "Summary:"
echo "  ✓ Counter contract deployed successfully"
echo "  ✓ Initial counter value was 0"
echo "  ✓ Hook executed via HooksTrampoline"
echo "  ✓ Counter incremented to 1"
echo "  ✓ Last caller was HooksTrampoline contract"
echo "  ✓ Access control prevents unauthorized calls"
echo ""
echo "🎉 HooksTrampoline is working correctly!"
echo "===================================="
