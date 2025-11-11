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
SETTLEMENT_CONTRACT="0x610178dA211FEF7D417bC0e6FeD39F05609AD788"
VAULT_RELAYER="0x6F1216D1BFe15c98520CA1434FC1d9D57AC95321"

# Two test accounts (Anvil defaults)
TRADER_A_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
TRADER_B_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"

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
  --value 2ether "deposit()" > /dev/null
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

# Create Order A: Sell 1 WETH for 2000 USDC
echo "📝 Creating Order A: Trader A sells 1 WETH for 2000 USDC"
echo ""
echo "  🚫 Note: Quote will likely return 'NoLiquidity' because we disabled Uniswap"
echo "  But we can still create the order manually for CoW matching!"
echo ""

# For PoC, we'll create a simple manual order without using the quote endpoint
# since it requires AMM liquidity to calculate prices

APPDATA='{"version":"1.3.0","metadata":{}}'
APP_DATA_HASH=$(echo -n "$APPDATA" | cast keccak)

# Upload app data first (need to escape the JSON properly for the fullAppData field)
echo "📤 Uploading app data..."
UPLOAD_RESPONSE=$(curl -s -X PUT "http://$HOST/api/v1/app_data/$APP_DATA_HASH" \
  -H 'Content-Type: application/json' \
  -d '{"fullAppData":"{\"version\":\"1.3.0\",\"metadata\":{}}"}')
echo "  Response: $UPLOAD_RESPONSE"
echo "  ✅ App data uploaded"
echo ""

# Order parameters
SELL_AMOUNT_A="1000000000000000000"  # 1 WETH (18 decimals)
BUY_AMOUNT_A="2000000000"             # 2000 USDC (6 decimals)
FEE_AMOUNT_A="0"                      # No fee for PoC
VALID_TO=$(( $(date +%s) + 3600 ))    # Valid for 1 hour

echo "  Sell: 1 WETH"
echo "  Buy: 2000 USDC"
echo "  Valid until: $VALID_TO"
echo ""

# Sign Order A using Python script
ORDER_A_UNSIGNED='{
  "sellToken": "'$WETH_ADDRESS'",
  "buyToken": "'$USDC_ADDRESS'",
  "receiver": "'$TRADER_A'",
  "sellAmount": "'$SELL_AMOUNT_A'",
  "buyAmount": "'$BUY_AMOUNT_A'",
  "validTo": '$VALID_TO',
  "appData": "'$APP_DATA_HASH'",
  "feeAmount": "'$FEE_AMOUNT_A'",
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

# Create Order B: Sell 2000 USDC for 1 WETH
echo "📝 Creating Order B: Trader B sells 2000 USDC for 1 WETH"
echo ""

ORDER_B_UNSIGNED='{
  "sellToken": "'$USDC_ADDRESS'",
  "buyToken": "'$WETH_ADDRESS'",
  "receiver": "'$TRADER_B'",
  "sellAmount": "'$BUY_AMOUNT_A'",
  "buyAmount": "'$SELL_AMOUNT_A'",
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

# Monitor for settlement by checking order status
SETTLED=false
for i in $(seq 1 24); do
  echo -n "  Checking settlement status... ($i/24)  \r"
  
  # Check if Order A is filled/settled
  ORDER_A_STATUS=$(curl -s "http://$HOST/api/v1/orders/$ORDER_A_UID" | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")
  
  if [ "$ORDER_A_STATUS" = "fulfilled" ] || [ "$ORDER_A_STATUS" = "executed" ]; then
    echo ""
    echo "  ✅ Orders settled successfully!"
    SETTLED=true
    break
  fi
  
  sleep 5
done

echo ""
if [ "$SETTLED" = true ]; then
  echo "🎉 SUCCESS! Orders were matched and settled!"
  echo ""
  echo "📊 Final balances:"
  FINAL_BALANCE_A_WETH=$(cast call $WETH_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_A)
  FINAL_BALANCE_A_USDC=$(cast call $USDC_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_A)
  FINAL_BALANCE_B_WETH=$(cast call $WETH_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_B)
  FINAL_BALANCE_B_USDC=$(cast call $USDC_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER_B)
  
  echo "  Trader A:"
  echo "    WETH: $(cast --from-wei $FINAL_BALANCE_A_WETH 2>/dev/null || echo $FINAL_BALANCE_A_WETH)"
  echo "    USDC: $(echo "scale=2; $FINAL_BALANCE_A_USDC / 1000000" | bc)"
  echo ""
  echo "  Trader B:"
  echo "    WETH: $(cast --from-wei $FINAL_BALANCE_B_WETH 2>/dev/null || echo $FINAL_BALANCE_B_WETH)"
  echo "    USDC: $(echo "scale=2; $FINAL_BALANCE_B_USDC / 1000000" | bc)"
else
  echo "⏸️  Orders not settled within 2 minutes"
  echo ""
  echo "📊 Debugging info:"
  echo "  Order A status: $ORDER_A_STATUS"
  echo ""
  echo "  Check logs for more details:"
  echo "    docker logs playground-autopilot-1 --tail 50"
  echo "    docker logs playground-baseline-1 --tail 50"
  echo "    docker logs playground-driver-1 --tail 50"
fi
