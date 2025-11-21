#!/bin/bash

# Parameterized test script for CoW Protocol offline mode
set -e
set -u

# Default values
HOST=localhost:8080
RPC_URL=http://localhost:8545
CHAIN_ID=31337

# Script directory for finding addresses.json
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDRESSES_FILE="$SCRIPT_DIR/offline-mode/config/addresses.json"

# Parse command line arguments
SELL_TOKEN=""
BUY_TOKEN=""
SELL_AMOUNT=""
BUY_AMOUNT=""
FROM_ADDRESS=""
TRADER_KEY=""

usage() {
  echo "Usage: $0 --sellToken <TOKEN> --buyToken <TOKEN> (--sellAmount <AMOUNT> | --buyAmount <AMOUNT>) --from <ADDRESS_OR_PRIVATE_KEY>"
  echo ""
  echo "Parameters:"
  echo "  --sellToken <TOKEN>       Token to sell (WETH, USDC, DAI, USDT, or GNO)"
  echo "  --buyToken <TOKEN>        Token to buy (WETH, USDC, DAI, USDT, or GNO)"
  echo "  --sellAmount <AMOUNT>     Amount to sell (e.g., 10e18, 1000e6) - mutually exclusive with --buyAmount"
  echo "  --buyAmount <AMOUNT>      Amount to buy (e.g., 10e18, 1000e6) - mutually exclusive with --sellAmount"
  echo "  --from <ADDRESS>          Trader address or private key (will be used as sender and receiver)"
  echo ""
  echo "Examples:"
  echo "  $0 --sellToken GNO --buyToken WETH --sellAmount 10e18 --from 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
  echo "  $0 --sellToken USDC --buyToken DAI --buyAmount 100e18 --from 0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
  exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --sellToken)
      SELL_TOKEN="$2"
      shift 2
      ;;
    --buyToken)
      BUY_TOKEN="$2"
      shift 2
      ;;
    --sellAmount)
      SELL_AMOUNT="$2"
      shift 2
      ;;
    --buyAmount)
      BUY_AMOUNT="$2"
      shift 2
      ;;
    --from)
      FROM_ADDRESS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Unknown parameter: $1"
      usage
      ;;
  esac
done

# Validate required parameters
if [ -z "$SELL_TOKEN" ] || [ -z "$BUY_TOKEN" ] || [ -z "$FROM_ADDRESS" ]; then
  echo "❌ Error: Missing required parameters"
  usage
fi

# Validate mutually exclusive sell/buy amount
if [ -n "$SELL_AMOUNT" ] && [ -n "$BUY_AMOUNT" ]; then
  echo "❌ Error: Cannot specify both --sellAmount and --buyAmount"
  usage
fi

if [ -z "$SELL_AMOUNT" ] && [ -z "$BUY_AMOUNT" ]; then
  echo "❌ Error: Must specify either --sellAmount or --buyAmount"
  usage
fi

# Determine if FROM_ADDRESS is a private key or address
if [[ $FROM_ADDRESS == 0x* ]] && [ ${#FROM_ADDRESS} -eq 66 ]; then
  # It's a private key
  TRADER_KEY="$FROM_ADDRESS"
  TRADER=$(cast wallet address $TRADER_KEY)
elif [[ $FROM_ADDRESS == 0x* ]] && [ ${#FROM_ADDRESS} -eq 42 ]; then
  # It's an address - error, we need the private key to sign
  echo "❌ Error: Please provide private key instead of address (needed for signing)"
  exit 1
else
  echo "❌ Error: Invalid address or private key format"
  exit 1
fi

echo "🧪 Testing CoW Protocol Offline Mode - Parameterized Order"
echo "==========================================================="
echo ""

# Load addresses from addresses.json
if [ ! -f "$ADDRESSES_FILE" ]; then
  echo "❌ Error: addresses.json not found at $ADDRESSES_FILE"
  exit 1
fi

# Function to get token address by symbol
get_token_address() {
  local token_symbol=$1
  local address=$(jq -r ".tokens.${token_symbol}" "$ADDRESSES_FILE")
  if [ "$address" = "null" ] || [ -z "$address" ]; then
    echo "❌ Error: Token $token_symbol not found in addresses.json"
    exit 1
  fi
  echo "$address"
}

# Function to get token decimals
get_token_decimals() {
  local token_symbol=$1
  case $token_symbol in
    WETH|DAI|GNO)
      echo 18
      ;;
    USDC|USDT)
      echo 6
      ;;
    *)
      echo "❌ Error: Unknown token $token_symbol"
      exit 1
      ;;
  esac
}

# Get token addresses
SELL_TOKEN_ADDRESS=$(get_token_address "$SELL_TOKEN")
BUY_TOKEN_ADDRESS=$(get_token_address "$BUY_TOKEN")
SELL_TOKEN_DECIMALS=$(get_token_decimals "$SELL_TOKEN")
BUY_TOKEN_DECIMALS=$(get_token_decimals "$BUY_TOKEN")

# Get contract addresses
SETTLEMENT_CONTRACT=$(jq -r '.cowProtocol.settlement' "$ADDRESSES_FILE")
VAULT_RELAYER=$(jq -r '.cowProtocol.vaultRelayer' "$ADDRESSES_FILE")

echo "Configuration:"
echo "  Trader: $TRADER"
echo "  Sell: $SELL_TOKEN ($SELL_TOKEN_ADDRESS) - $SELL_TOKEN_DECIMALS decimals"
echo "  Buy: $BUY_TOKEN ($BUY_TOKEN_ADDRESS) - $BUY_TOKEN_DECIMALS decimals"
echo "  Settlement: $SETTLEMENT_CONTRACT"
echo ""

# Parse amount (handle scientific notation like 10e18)
parse_amount() {
  local amount=$1
  # If amount contains 'e', parse it as scientific notation
  if [[ $amount == *e* ]]; then
    # Use bc for calculation
    echo "$amount" | awk '{printf "%.0f\n", $1}'
  else
    echo "$amount"
  fi
}

# Wait for services
echo "⏳ Waiting for services to be ready..."
curl --retry 24 --retry-delay 5 --retry-all-errors --fail-with-body -s \
  http://$HOST/api/v1/version > /dev/null
echo "✅ Services ready!"
echo ""

# Setup trader with tokens
echo "💰 Setting up trader with tokens..."

# Function to mint/deposit tokens based on type
setup_token_balance() {
  local token=$1
  local token_address=$2
  local amount=$3

  if [ "$token" = "WETH" ]; then
    # Deposit ETH to get WETH
    cast send $token_address --private-key $TRADER_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
      --value $amount "deposit()" > /dev/null
  else
    # Mint ERC20 tokens
    cast send $token_address --private-key $TRADER_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
      "mint(address,uint256)" $TRADER $amount > /dev/null
  fi
}

# Ensure trader has enough sell token
if [ -n "$SELL_AMOUNT" ]; then
  PARSED_SELL_AMOUNT=$(parse_amount "$SELL_AMOUNT")
  # Add some buffer for testing
  SETUP_AMOUNT=$(echo "$PARSED_SELL_AMOUNT * 2" | bc)
  setup_token_balance "$SELL_TOKEN" "$SELL_TOKEN_ADDRESS" "$SETUP_AMOUNT"
else
  # If buyAmount is specified, we still need some sell tokens (estimate 10x)
  PARSED_BUY_AMOUNT=$(parse_amount "$BUY_AMOUNT")
  # Rough estimate: assume 1:1 ratio and add buffer
  SETUP_AMOUNT=$(echo "$PARSED_BUY_AMOUNT * 10" | bc | awk '{printf "%.0f\n", $1}')
  setup_token_balance "$SELL_TOKEN" "$SELL_TOKEN_ADDRESS" "$SETUP_AMOUNT"
fi

# Approve vault relayer to spend tokens
cast send $SELL_TOKEN_ADDRESS --private-key $TRADER_KEY --rpc-url $RPC_URL --chain-id $CHAIN_ID \
  "approve(address,uint256)" $VAULT_RELAYER "1000000000000000000000000" > /dev/null

# Check balance
BALANCE=$(cast call $SELL_TOKEN_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER)
if [ "$SELL_TOKEN_DECIMALS" -eq 18 ]; then
  echo "  ✅ Trader has $(cast --from-wei $BALANCE) $SELL_TOKEN"
else
  echo "  ✅ Trader has $(echo "scale=2; $BALANCE / 10^$SELL_TOKEN_DECIMALS" | bc) $SELL_TOKEN"
fi
echo ""

# Determine order kind and amount
if [ -n "$SELL_AMOUNT" ]; then
  ORDER_KIND="sell"
  AMOUNT_FIELD="sellAmountBeforeFee"
  AMOUNT_VALUE=$(parse_amount "$SELL_AMOUNT")
  echo "📝 Creating order: Sell $SELL_AMOUNT $SELL_TOKEN for $BUY_TOKEN"
else
  ORDER_KIND="buy"
  AMOUNT_FIELD="buyAmountAfterFee"
  AMOUNT_VALUE=$(parse_amount "$BUY_AMOUNT")
  echo "📝 Creating order: Buy $BUY_AMOUNT $BUY_TOKEN with $SELL_TOKEN"
fi
echo ""

# Get quote from API
echo "📊 Getting quote from orderbook..."
QUOTE_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/quote" \
  -H 'Content-Type: application/json' \
  -d '{
    "sellToken": "'$SELL_TOKEN_ADDRESS'",
    "buyToken": "'$BUY_TOKEN_ADDRESS'",
    "receiver": "'$TRADER'",
    "'$AMOUNT_FIELD'": "'$AMOUNT_VALUE'",
    "kind": "'$ORDER_KIND'",
    "from": "'$TRADER'"
  }')

# Check if quote was successful
if echo "$QUOTE_RESPONSE" | jq -e '.errorType' > /dev/null 2>&1; then
  echo "❌ Quote request failed:"
  echo "$QUOTE_RESPONSE" | jq .
  exit 1
fi

# Extract quote parameters
QUOTE_SELL_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.sellAmount')
QUOTE_BUY_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.buyAmount')
FEE_AMOUNT=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.feeAmount')
VALID_TO=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.validTo')
APP_DATA_HASH=$(echo "$QUOTE_RESPONSE" | jq -r '.quote.appData')

# Validate that we got valid values
if [ "$QUOTE_BUY_AMOUNT" = "null" ] || [ "$VALID_TO" = "null" ]; then
  echo "❌ Quote returned null values:"
  echo "$QUOTE_RESPONSE" | jq .
  exit 1
fi

echo "  Quote received:"
if [ "$SELL_TOKEN_DECIMALS" -eq 18 ]; then
  echo "    Sell amount: $(cast --from-wei $QUOTE_SELL_AMOUNT 2>/dev/null || echo $QUOTE_SELL_AMOUNT) $SELL_TOKEN"
else
  echo "    Sell amount: $(echo "scale=2; $QUOTE_SELL_AMOUNT / 10^$SELL_TOKEN_DECIMALS" | bc) $SELL_TOKEN"
fi
if [ "$BUY_TOKEN_DECIMALS" -eq 18 ]; then
  echo "    Buy amount: $(cast --from-wei $QUOTE_BUY_AMOUNT 2>/dev/null || echo $QUOTE_BUY_AMOUNT) $BUY_TOKEN"
else
  echo "    Buy amount: $(echo "scale=2; $QUOTE_BUY_AMOUNT / 10^$BUY_TOKEN_DECIMALS" | bc) $BUY_TOKEN"
fi
echo "    Valid until: $VALID_TO"
echo ""

# Sign Order using Python script (fee set to 0 for offline mode)
ORDER_UNSIGNED='{
  "sellToken": "'$SELL_TOKEN_ADDRESS'",
  "buyToken": "'$BUY_TOKEN_ADDRESS'",
  "receiver": "'$TRADER'",
  "sellAmount": "'$QUOTE_SELL_AMOUNT'",
  "buyAmount": "'$QUOTE_BUY_AMOUNT'",
  "validTo": '$VALID_TO',
  "appData": "'$APP_DATA_HASH'",
  "feeAmount": "0",
  "kind": "'$ORDER_KIND'",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "chainId": '$CHAIN_ID',
  "settlement": "'$SETTLEMENT_CONTRACT'"
}'

ORDER_SIGNED=$(python3 offline-mode/scripts/sign_order.py "$TRADER_KEY" "$ORDER_UNSIGNED")

echo "📤 Posting order..."
ORDER_RESPONSE=$(curl -s -X POST "http://$HOST/api/v1/orders" \
  -H 'Content-Type: application/json' \
  -d "$ORDER_SIGNED" || echo "failed")

if [ "$ORDER_RESPONSE" = "failed" ]; then
  echo "❌ Failed to post order"
  exit 1
fi

echo "Response: $ORDER_RESPONSE"
echo ""

ORDER_UID=$(echo "$ORDER_RESPONSE" | tr -d '"')
echo "  Order UID: ${ORDER_UID:0:20}..."
echo ""

echo "⏳ Waiting for order to be settled..."
echo "   (Checking every 5 seconds for up to 2 minutes)"
echo ""

# Function to check order status
check_order_status() {
  local order_uid=$1
  local order_data=$(curl -s "http://$HOST/api/v1/orders/$order_uid" 2>/dev/null)
  echo "$order_data" | jq -r '.status' 2>/dev/null || echo "unknown"
}

# Wait for order to be settled (max 2 minutes)
MAX_WAIT=120
ELAPSED=0
SETTLEMENT_FOUND=false

while [ $ELAPSED -lt $MAX_WAIT ]; do
  ORDER_STATUS=$(check_order_status "$ORDER_UID")

  echo "  [$ELAPSED s] Order status: $ORDER_STATUS"

  # Check if order is fulfilled/traded
  if [ "$ORDER_STATUS" = "fulfilled" ] || [ "$ORDER_STATUS" = "traded" ]; then
    SETTLEMENT_FOUND=true
    echo ""
    echo "✅ Order settled successfully!"
    break
  fi

  sleep 5
  ELAPSED=$((ELAPSED + 5))
done

if [ "$SETTLEMENT_FOUND" = false ]; then
  echo ""
  echo "❌ ERROR: Order was not settled within $MAX_WAIT seconds"
  echo ""
  echo "Final status: $ORDER_STATUS"
  echo ""
  echo "Driver logs (last 30 lines):"
  docker-compose -f docker-compose.offline.yml logs driver --tail=30 2>&1 | grep -E "(error|Error|ERROR|settlement|solution)" || echo "No relevant logs found"
  exit 1
fi

echo ""
echo "📊 Checking final balances..."
echo ""

# Function to parse balance and remove brackets/scientific notation
parse_balance() {
  local raw_value=$1
  echo "$raw_value" | sed 's/\[//g' | sed 's/\]//g' | awk '{print $1}'
}

# Check final balances
FINAL_BALANCE_SELL_RAW=$(cast call $SELL_TOKEN_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER)
FINAL_BALANCE_BUY_RAW=$(cast call $BUY_TOKEN_ADDRESS --rpc-url $RPC_URL "balanceOf(address)(uint256)" $TRADER)

FINAL_BALANCE_SELL=$(parse_balance "$FINAL_BALANCE_SELL_RAW")
FINAL_BALANCE_BUY=$(parse_balance "$FINAL_BALANCE_BUY_RAW")

echo "Trader final balances:"
if [ "$SELL_TOKEN_DECIMALS" -eq 18 ]; then
  echo "  $SELL_TOKEN: $(cast --to-unit $FINAL_BALANCE_SELL ether 2>/dev/null || echo "$(echo "scale=4; $FINAL_BALANCE_SELL / 1000000000000000000" | bc)")"
else
  echo "  $SELL_TOKEN: $(echo "scale=2; $FINAL_BALANCE_SELL / 10^$SELL_TOKEN_DECIMALS" | bc)"
fi
if [ "$BUY_TOKEN_DECIMALS" -eq 18 ]; then
  echo "  $BUY_TOKEN: $(cast --to-unit $FINAL_BALANCE_BUY ether 2>/dev/null || echo "$(echo "scale=4; $FINAL_BALANCE_BUY / 1000000000000000000" | bc)")"
else
  echo "  $BUY_TOKEN: $(echo "scale=2; $FINAL_BALANCE_BUY / 10^$BUY_TOKEN_DECIMALS" | bc)"
fi
echo ""

# Verify trader received buy tokens
BUY_TOKEN_GT_ZERO=$(echo "$FINAL_BALANCE_BUY > 0" | bc)

if [ "$BUY_TOKEN_GT_ZERO" = "1" ]; then
  echo "✅ Trade executed successfully!"
else
  echo "❌ Trade did not execute as expected"
  echo "   Buy token balance: $FINAL_BALANCE_BUY"
  exit 1
fi

echo ""
echo "🎉 TEST PASSED - Order settled successfully!"
echo "==========================================="
