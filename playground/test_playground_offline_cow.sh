#!/bin/bash

# Test CoW Protocol offline mode with peer-to-peer order matching (no AMM needed)
set -e
set -u

HOST=localhost:8080
RPC_URL=http://localhost:8545
CHAIN_ID=31337

# Contract addresses from addresses.json
WETH_ADDRESS="0x5FbDB2315678afecb367f032d93F642f64180aa3"
USDC_ADDRESS="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
SETTLEMENT_CONTRACT="0xB7f8BC63BbcaD18155201308C8f3540b07f84F5e"
VAULT_RELAYER="0x8dAF17A20c9DBA35f005b6324F493785D239719d"
VAULT="0x8A791620dd6260079BF849Dc5567aDC3F2FdC318"

# Two test accounts (Anvil accounts #1 and #2 - not using #0 since it's the solver)
TRADER_A_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"  # Account #1
TRADER_B_KEY="0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"  # Account #2

TRADER_A=$(cast wallet address $TRADER_A_KEY)
TRADER_B=$(cast wallet address $TRADER_B_KEY)

echo "🧪 Testing CoW Protocol Offline Mode - Peer-to-Peer Matching"
echo "============================================================="
echo ""
echo "Trader A: $TRADER_A (will sell WETH for USDC)"
echo "Trader B: $TRADER_B (will sell USDC for WETH)"
echo ""

# Wait for services
echo "⏳ Waiting for services to be ready..."
curl --retry 24 --retry-delay 5 --retry-all-errors --fail-with-body -s \
  http://$HOST/api/v1/version > /dev/null
echo "✅ Services ready!"
echo ""

# Setup Trader A (has WETH, wants USDC)
echo "💰 Setting up Trader A with WETH..."
cast send $WETH_ADDRESS --private-key $TRADER_A_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
  --value 2000000000000000000 "deposit()" > /dev/null
cast send $WETH_ADDRESS --private-key $TRADER_A_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
  "approve(address,uint256)" $VAULT_RELAYER "1000000000000000000000000" > /dev/null
BALANCE_A=$(cast call $WETH_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_A)
echo "  ✅ Trader A has $(cast --from-wei $BALANCE_A) WETH"
echo ""

# Setup Trader B (has USDC, wants WETH)
echo "💰 Setting up Trader B with USDC..."
# Mint USDC to Trader B (20,000 USDC - 6 decimals)
cast send $USDC_ADDRESS --private-key $TRADER_B_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
  "mint(address,uint256)" $TRADER_B "20000000000" > /dev/null
cast send $USDC_ADDRESS --private-key $TRADER_B_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
  "approve(address,uint256)" $VAULT_RELAYER "1000000000000000000000000" > /dev/null
BALANCE_B=$(cast call $USDC_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_B)
echo "  ✅ Trader B has $(echo "scale=2; $BALANCE_B / 1000000" | bc) USDC"
echo ""

# Create Order A: Sell 0.1 WETH for USDC using quote endpoint
echo "📝 Creating Order A: Trader A sells 0.1 WETH for USDC"
echo ""

# Get quote from API
echo "📊 Getting quote from orderbook..."
QUOTE_A_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/quote" \
  -H 'Content-Type: application/json' \
  -d '{
    "sellToken": "'$WETH_ADDRESS'",
    "buyToken": "'$USDC_ADDRESS'",
    "receiver": "'$TRADER_A'",
    "sellAmountBeforeFee": "100000000000000000",
    "kind": "sell",
    "from": "'$TRADER_A'"
  }')

# Check if quote was successful
if echo "$QUOTE_A_RESPONSE" | jq -e '.errorType' > /dev/null 2>&1; then
  echo "❌ Quote request failed:"
  echo "$QUOTE_A_RESPONSE" | jq .
  exit 1
fi

# Extract quote parameters
SELL_AMOUNT_A=$(echo "$QUOTE_A_RESPONSE" | jq -r '.quote.sellAmount')
BUY_AMOUNT_A=$(echo "$QUOTE_A_RESPONSE" | jq -r '.quote.buyAmount')
FEE_AMOUNT_A=$(echo "$QUOTE_A_RESPONSE" | jq -r '.quote.feeAmount')
VALID_TO=$(echo "$QUOTE_A_RESPONSE" | jq -r '.quote.validTo')
APP_DATA_HASH=$(echo "$QUOTE_A_RESPONSE" | jq -r '.quote.appData')

# Validate that we got valid values
if [ "$BUY_AMOUNT_A" = "null" ] || [ "$VALID_TO" = "null" ]; then
  echo "❌ Quote returned null values:"
  echo "$QUOTE_A_RESPONSE" | jq .
  exit 1
fi

echo "  Quote received:"
echo "    Sell amount: $(cast --from-wei $SELL_AMOUNT_A 2>/dev/null || echo $SELL_AMOUNT_A) WETH"
echo "    Buy amount: $(echo "scale=2; $BUY_AMOUNT_A / 1000000" | bc) USDC"
echo "    Fee: $(cast --from-wei $FEE_AMOUNT_A 2>/dev/null || echo $FEE_AMOUNT_A) WETH"
echo "    Valid until: $VALID_TO"
echo ""

# Sign Order A using Python script (fee set to 0 for offline mode)
ORDER_A_UNSIGNED='{
  "sellToken": "'$WETH_ADDRESS'",
  "buyToken": "'$USDC_ADDRESS'",
  "receiver": "'$TRADER_A'",
  "sellAmount": "100000000000000000",
  "buyAmount": "'$BUY_AMOUNT_A'",
  "validTo": '$VALID_TO',
  "appData": "'$APP_DATA_HASH'",
  "feeAmount": "0",
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "chainId": '$CHAIN_ID',
  "settlement": "'$SETTLEMENT_CONTRACT'"
}'

ORDER_A_SIGNED=$(python3 poc-offline-mode/scripts/sign_order.py "$TRADER_A_KEY" "$ORDER_A_UNSIGNED")

echo "📤 Posting Order A..."
ORDER_A_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/orders" \
  -H 'Content-Type: application/json' \
  -d "$ORDER_A_SIGNED" || echo "failed")

echo "Response: $ORDER_A_RESPONSE"
echo ""

# Create Order B: Sell USDC for WETH using quote endpoint
echo "📝 Creating Order B: Trader B sells USDC for WETH"
echo ""

# Get quote from API
echo "📊 Getting quote from orderbook..."
QUOTE_B_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/quote" \
  -H 'Content-Type: application/json' \
  -d '{
    "sellToken": "'$USDC_ADDRESS'",
    "buyToken": "'$WETH_ADDRESS'",
    "receiver": "'$TRADER_B'",
    "sellAmountBeforeFee": "200000000",
    "kind": "sell",
    "from": "'$TRADER_B'"
  }')

# Check if quote was successful
if echo "$QUOTE_B_RESPONSE" | jq -e '.errorType' > /dev/null 2>&1; then
  echo "❌ Quote request failed:"
  echo "$QUOTE_B_RESPONSE" | jq .
  exit 1
fi

# Extract quote parameters
SELL_AMOUNT_B=$(echo "$QUOTE_B_RESPONSE" | jq -r '.quote.sellAmount')
BUY_AMOUNT_B=$(echo "$QUOTE_B_RESPONSE" | jq -r '.quote.buyAmount')
FEE_AMOUNT_B=$(echo "$QUOTE_B_RESPONSE" | jq -r '.quote.feeAmount')
VALID_TO_B=$(echo "$QUOTE_B_RESPONSE" | jq -r '.quote.validTo')
APP_DATA_HASH_B=$(echo "$QUOTE_B_RESPONSE" | jq -r '.quote.appData')

# Validate that we got valid values
if [ "$BUY_AMOUNT_B" = "null" ] || [ "$VALID_TO_B" = "null" ]; then
  echo "❌ Quote returned null values:"
  echo "$QUOTE_B_RESPONSE" | jq .
  exit 1
fi

echo "  Quote received:"
echo "    Sell amount: $(echo "scale=2; $SELL_AMOUNT_B / 1000000" | bc) USDC"
echo "    Buy amount: $(cast --from-wei $BUY_AMOUNT_B 2>/dev/null || echo $BUY_AMOUNT_B) WETH"
echo "    Fee: $(echo "scale=2; $FEE_AMOUNT_B / 1000000" | bc) USDC"
echo "    Valid until: $VALID_TO_B"
echo ""

# Sign Order B with fee set to 0 for offline mode
ORDER_B_UNSIGNED='{
  "sellToken": "'$USDC_ADDRESS'",
  "buyToken": "'$WETH_ADDRESS'",
  "receiver": "'$TRADER_B'",
  "sellAmount": "200000000",
  "buyAmount": "'$BUY_AMOUNT_B'",
  "validTo": '$VALID_TO_B',
  "appData": "'$APP_DATA_HASH_B'",
  "feeAmount": "0",
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "chainId": '$CHAIN_ID',
  "settlement": "'$SETTLEMENT_CONTRACT'"
}'

ORDER_B_SIGNED=$(python3 poc-offline-mode/scripts/sign_order.py "$TRADER_B_KEY" "$ORDER_B_UNSIGNED")

echo "📤 Posting Order B..."
ORDER_B_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/orders" \
  -H 'Content-Type: application/json' \
  -d "$ORDER_B_SIGNED" || echo "failed")

echo "Response: $ORDER_B_RESPONSE"
echo ""

echo "⏳ Waiting for autopilot to match orders and create settlement..."
echo "   (Checking every 5 seconds for up to 2 minutes)"
echo ""

# Extract order UIDs from responses
ORDER_A_UID=$(echo "$ORDER_A_RESPONSE" | tr -d '"')
ORDER_B_UID=$(echo "$ORDER_B_RESPONSE" | tr -d '"')

echo "  Order A UID: ${ORDER_A_UID:0:20}..."
echo "  Order B UID: ${ORDER_B_UID:0:20}..."
echo ""

# Function to check order status
check_order_status() {
  local order_uid=$1
  local order_data=$(curl -s "http://$HOST/api/v1/orders/$order_uid" 2>/dev/null)
  echo "$order_data" | jq -r '.status' 2>/dev/null || echo "unknown"
}

# Wait for orders to be settled (max 2 minutes)
MAX_WAIT=120
ELAPSED=0
SETTLEMENT_FOUND=false

while [ $ELAPSED -lt $MAX_WAIT ]; do
  ORDER_A_STATUS=$(check_order_status "$ORDER_A_UID")
  ORDER_B_STATUS=$(check_order_status "$ORDER_B_UID")

  echo "  [$ELAPSED s] Order A: $ORDER_A_STATUS | Order B: $ORDER_B_STATUS"

  # Check if BOTH orders are fulfilled/traded (peer-to-peer matching)
  if ([ "$ORDER_A_STATUS" = "fulfilled" ] || [ "$ORDER_A_STATUS" = "traded" ]) && \
     ([ "$ORDER_B_STATUS" = "fulfilled" ] || [ "$ORDER_B_STATUS" = "traded" ]); then
    SETTLEMENT_FOUND=true
    echo ""
    echo "✅ Both orders settled successfully!"
    break
  fi

  sleep 5
  ELAPSED=$((ELAPSED + 5))
done

if [ "$SETTLEMENT_FOUND" = false ]; then
  echo ""
  echo "❌ ERROR: Orders were not settled within $MAX_WAIT seconds"
  echo ""
  echo "Final status:"
  echo "  Order A: $ORDER_A_STATUS"
  echo "  Order B: $ORDER_B_STATUS"
  echo ""
  echo "Driver logs (last 30 lines):"
  docker-compose -f docker-compose.offline.yml logs driver --tail=30 2>&1 | grep -E "(error|Error|ERROR|settlement|solution)" || echo "No relevant logs found"
  exit 1
fi

echo ""
echo "🔍 Verifying settlement transaction..."

# Get settlement transaction from driver logs
SETTLEMENT_TX=$(docker-compose -f docker-compose.offline.yml logs driver --since 3m 2>&1 | \
  grep -oE "0x[a-fA-F0-9]{64}" | grep -v "0x0000000000000000000000000000000000000000000000000000000000000000" | tail -1)

if [ -n "$SETTLEMENT_TX" ]; then
  echo "  Settlement TX: $SETTLEMENT_TX"
fi

echo ""
echo "📊 Checking final balances..."
echo ""

# Function to parse balance and remove brackets/scientific notation
parse_balance() {
  local raw_value=$1
  # Remove brackets and whitespace, extract just the number
  echo "$raw_value" | sed 's/\[//g' | sed 's/\]//g' | awk '{print $1}'
}

# Check final balances
FINAL_BALANCE_A_WETH_RAW=$(cast call $WETH_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_A)
FINAL_BALANCE_A_USDC_RAW=$(cast call $USDC_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_A)
FINAL_BALANCE_B_WETH_RAW=$(cast call $WETH_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_B)
FINAL_BALANCE_B_USDC_RAW=$(cast call $USDC_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_B)

FINAL_BALANCE_A_WETH=$(parse_balance "$FINAL_BALANCE_A_WETH_RAW")
FINAL_BALANCE_A_USDC=$(parse_balance "$FINAL_BALANCE_A_USDC_RAW")
FINAL_BALANCE_B_WETH=$(parse_balance "$FINAL_BALANCE_B_WETH_RAW")
FINAL_BALANCE_B_USDC=$(parse_balance "$FINAL_BALANCE_B_USDC_RAW")

BALANCE_A_CLEAN=$(parse_balance "$BALANCE_A")
BALANCE_B_CLEAN=$(parse_balance "$BALANCE_B")

echo "Trader A (sold WETH for USDC):"
echo "  WETH: $(cast --to-unit $FINAL_BALANCE_A_WETH ether 2>/dev/null || echo "$(echo "scale=4; $FINAL_BALANCE_A_WETH / 1000000000000000000" | bc) ETH") (was: $(cast --to-unit $BALANCE_A_CLEAN ether 2>/dev/null || echo "$(echo "scale=4; $BALANCE_A_CLEAN / 1000000000000000000" | bc) ETH"))"
echo "  USDC: $(echo "scale=2; $FINAL_BALANCE_A_USDC / 1000000" | bc)"
echo ""
echo "Trader B (sold USDC for WETH):"
echo "  WETH: $(cast --to-unit $FINAL_BALANCE_B_WETH ether 2>/dev/null || echo "$(echo "scale=4; $FINAL_BALANCE_B_WETH / 1000000000000000000" | bc) ETH")"
echo "  USDC: $(echo "scale=2; $FINAL_BALANCE_B_USDC / 1000000" | bc) (was: $(echo "scale=2; $BALANCE_B_CLEAN / 1000000" | bc))"
echo ""

# Verify balances changed (use bc for comparison to handle large numbers)
A_USDC_GT_ZERO=$(echo "$FINAL_BALANCE_A_USDC > 0" | bc)
B_WETH_GT_ZERO=$(echo "$FINAL_BALANCE_B_WETH > 0" | bc)

if [ "$A_USDC_GT_ZERO" = "1" ] && [ "$B_WETH_GT_ZERO" = "1" ]; then
  echo "✅ Balances updated correctly - trades executed!"
else
  echo "❌ Balances did not change as expected"
  echo "   Trader A USDC: $FINAL_BALANCE_A_USDC"
  echo "   Trader B WETH: $FINAL_BALANCE_B_WETH"
  exit 1
fi

echo ""
echo "🎉 TEST PASSED - CoW Protocol offline mode working correctly!"
echo "============================================================="
