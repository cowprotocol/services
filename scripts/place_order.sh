#!/bin/bash
# =============================================================================
# CoW Protocol — Place a sell order via the Orderbook API
#
# Usage: ./scripts/place_order.sh
#
# Prerequisites:
#   - Anvil running at http://localhost:8545 (fork of mainnet)
#   - All services running (baseline solver, driver, orderbook, autopilot)
#   - `cast`, `jq`, and `python3` installed
# =============================================================================

set -euo pipefail

# --- Configuration ---
HOST="${HOST:-localhost:8080}"
RPC_URL="${RPC_URL:-http://localhost:8545}"

WETH_ADDRESS="0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
SELL_TOKEN="$WETH_ADDRESS"
BUY_TOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"  # USDC
SELL_AMOUNT="1000000000000000000"  # 1 ETH (in wei)
SLIPPAGE=5  # 5% slippage tolerance

# GPv2Settlement contract on mainnet
SETTLEMENT_CONTRACT="0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
# GPv2VaultRelayer contract on mainnet
VAULT_RELAYER="0xC92E8bdf79f0507f65a392b0ab4667716BFE0110"

# Known Anvil test private key (account index 9)
PRIVATE_KEY="0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"

# Use zero appData (simplest, always accepted)
APP_DATA="0x0000000000000000000000000000000000000000000000000000000000000000"

# --- Derived values ---
SENDER=$(cast wallet address "$PRIVATE_KEY")

echo "=== CoW Protocol Order Placement ==="
echo "Sender:     $SENDER"
echo "Sell token: $SELL_TOKEN (WETH)"
echo "Buy token:  $BUY_TOKEN (USDC)"
echo "Sell amount: $SELL_AMOUNT wei (1 ETH)"
echo ""

# --- Step 0: Wait for the orderbook to be ready ---
echo "Waiting for orderbook to be ready..."
for i in $(seq 1 30); do
  if curl -s -f "http://$HOST/api/v1/token/$BUY_TOKEN/native_price" > /dev/null 2>&1; then
    echo "Orderbook is ready!"
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo "ERROR: Orderbook not ready after 150s"
    exit 1
  fi
  sleep 5
done

# --- Step 1: Wrap ETH → WETH ---
echo ""
echo "Step 1: Wrapping $SELL_AMOUNT wei ETH → WETH..."
cast send --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" \
  "$WETH_ADDRESS" "deposit()" --value "$SELL_AMOUNT" > /dev/null 2>&1
echo "  ✅ Wrapped ETH → WETH"

# --- Step 2: Approve WETH for the vault relayer ---
echo ""
echo "Step 2: Approving WETH for vault relayer..."
cast send --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" \
  "$WETH_ADDRESS" "approve(address,uint256)" \
  "$VAULT_RELAYER" "$SELL_AMOUNT" > /dev/null 2>&1
echo "  ✅ Approved WETH spending"

# --- Step 3: Get a quote ---
echo ""
echo "Step 3: Getting price quote..."
QUOTE_RESPONSE=$(curl --retry 3 -s -X 'POST' \
  "http://$HOST/api/v1/quote" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'"$SELL_TOKEN"'",
  "buyToken": "'"$BUY_TOKEN"'",
  "from": "'"$SENDER"'",
  "receiver": "'"$SENDER"'",
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "onchainOrder": false,
  "partiallyFillable": false,
  "kind": "sell",
  "sellAmountBeforeFee": "'"$SELL_AMOUNT"'"
}')

echo "  Quote response (truncated): $(echo "$QUOTE_RESPONSE" | head -c 400)"
echo ""

QUOTE_ID=$(echo "$QUOTE_RESPONSE" | jq -r '.id')
BUY_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.buyAmount')
QUOTED_SELL_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.sellAmount')
QUOTED_FEE_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.feeAmount')
VALID_TO=$(($(date +%s) + 600))  # valid for 10 minutes

if [ -z "$BUY_AMOUNT" ] || [ "$BUY_AMOUNT" = "null" ]; then
  echo "ERROR: Failed to get a valid quote"
  echo "Full response: $QUOTE_RESPONSE"
  exit 1
fi

# Apply slippage to buy amount
BUY_AMOUNT_WITH_SLIPPAGE=$(python3 -c "print(int(int('$BUY_AMOUNT') * (100 - $SLIPPAGE) / 100))")

echo "  Quote ID:    $QUOTE_ID"
echo "  Buy amount:  $BUY_AMOUNT (before slippage)"
echo "  Buy amount:  $BUY_AMOUNT_WITH_SLIPPAGE (after ${SLIPPAGE}% slippage)"
echo "  Sell amount: $QUOTED_SELL_AMOUNT"
echo "  Fee amount:  $QUOTED_FEE_AMOUNT"
echo "  Valid until: $VALID_TO"

# --- Step 4: Place the order ---
echo ""
echo "Step 4: Placing order..."

# NOTE: feeAmount is "0" because fees are included in the sell amount.
# appData uses the zero hash (simplest). No appDataHash field needed.
ORDER_RESPONSE=$(curl -s -w "\n%{http_code}" -X 'POST' \
  "http://$HOST/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'"$SELL_TOKEN"'",
  "buyToken": "'"$BUY_TOKEN"'",
  "receiver": "'"$SENDER"'",
  "sellAmount": "'"$QUOTED_SELL_AMOUNT"'",
  "buyAmount": "'"$BUY_AMOUNT_WITH_SLIPPAGE"'",
  "validTo": '"$VALID_TO"',
  "feeAmount": "0",
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "presign",
  "signature": "'"$SENDER"'",
  "from": "'"$SENDER"'",
  "appData": "'"$APP_DATA"'"
}')

# Parse response body and HTTP status
HTTP_STATUS=$(echo "$ORDER_RESPONSE" | tail -1)
RESPONSE_BODY=$(echo "$ORDER_RESPONSE" | sed '$d')

echo "  HTTP status: $HTTP_STATUS"
echo "  Response: $RESPONSE_BODY"

if [ "$HTTP_STATUS" != "201" ]; then
  echo "ERROR: Failed to place order (HTTP $HTTP_STATUS)"
  echo "Full response: $RESPONSE_BODY"
  exit 1
fi

# Extract order UID (it's a quoted string)
ORDER_UID=$(echo "$RESPONSE_BODY" | jq -r '.')
echo "  ✅ Order UID: $ORDER_UID"

# --- Step 4b: Send presign transaction ---
echo ""
echo "Step 4b: Sending presign transaction to settlement contract..."
cast send --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" \
  "$SETTLEMENT_CONTRACT" \
  "setPreSignature(bytes,bool)" \
  "$ORDER_UID" true > /dev/null 2>&1
echo "  ✅ Presign transaction sent"

# --- Step 5: Poll order status ---
echo ""
echo "Step 5: Polling order status..."
for i in $(seq 1 60); do
  ORDER_STATUS=$(curl -s -X 'GET' \
    "http://$HOST/api/v1/orders/$ORDER_UID/status" \
    -H 'accept: application/json' 2>/dev/null | jq -r '.type' 2>/dev/null || echo "unknown")

  printf "  [%02d/60] Order status: %-15s\n" "$i" "$ORDER_STATUS"

  if [ "$ORDER_STATUS" = "traded" ] || [ "$ORDER_STATUS" = "fulfilled" ]; then
    echo ""
    echo "🎉 SUCCESS! Order was filled!"
    echo "Order UID: $ORDER_UID"
    exit 0
  fi

  if [ "$ORDER_STATUS" = "cancelled" ] || [ "$ORDER_STATUS" = "expired" ]; then
    echo ""
    echo "❌ Order $ORDER_STATUS"
    exit 1
  fi

  sleep 5
done

echo ""
echo "⏰ Timeout waiting for order to be filled (5 minutes)"
echo "Order UID: $ORDER_UID"
echo "Current status: $ORDER_STATUS"
echo ""
echo "The order may still be filled. Check manually:"
echo "  curl http://$HOST/api/v1/orders/$ORDER_UID/status"
exit 1
