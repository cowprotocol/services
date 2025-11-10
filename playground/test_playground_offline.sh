#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
# Use locally deployed tokens from .env.offline
WETH_ADDRESS="0xc6e7DF5E7b4f2A278906862b61205850344D4e7d"  # Local WETH
SELL_TOKEN=$WETH_ADDRESS
BUY_TOKEN="0x4ed7c70F96B99c776995fB64377f0d4aB3B0e1C1"  # Local USDC
SELL_AMOUNT="1000000000000000000"  # 1 WETH
SLIPPAGE=2  # 2%
SETTLEMENT_CONTRACT="0xE6E340D132b5f46d1e472DebcD681B2aBc16e57E"  # Local Settlement
APPDATA='{"version":"1.3.0","metadata":{}}'

# Following private key is only used for testing purposes in a local environment.
# This is Anvil's default account #0 private key
PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# Wait for services to be ready
echo "Waiting until all services are ready"
curl --retry 24 --retry-delay 5 --retry-all-errors --fail-with-body -s --show-error \
  -H 'accept:application/json' \
  http://$HOST/api/v1/version > /dev/null

echo "Services are ready!"

# Run test flow
echo "Using private key:" $PRIVATE_KEY
receiver=$(cast wallet address $PRIVATE_KEY)
echo "Receiver address: $receiver"

# Calculate AppData hash
app_data_hash=$(cast keccak $APPDATA)
echo "AppData hash: $app_data_hash"

# First, we need to wrap some ETH and approve the WETH
echo "Wrapping ETH to WETH..."
cast send $WETH_ADDRESS "deposit()" \
  --rpc-url http://localhost:8545 \
  --private-key $PRIVATE_KEY \
  --chain-id 31337 \
  --value $SELL_AMOUNT > /dev/null

echo "Approving WETH for Settlement contract..."
cast send $WETH_ADDRESS "approve(address,uint256)" \
  $SETTLEMENT_CONTRACT \
  "1000000000000000000000000" \
  --rpc-url http://localhost:8545 \
  --private-key $PRIVATE_KEY \
  --chain-id 31337 > /dev/null

echo "Request price quote for buying USDC with WETH"
quote_response=$( curl --retry 5 --fail-with-body -s --show-error -X 'POST' \
  "http://$HOST/api/v1/quote" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'$SELL_TOKEN'",
  "buyToken": "'$BUY_TOKEN'",
  "from": "'$receiver'",
  "receiver": "'$receiver'",
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "onchainOrder": false,
  "partiallyFillable": false,
  "kind": "sell",
  "sellAmountBeforeFee": "'$SELL_AMOUNT'"
}')

echo "Quote response:"
echo $quote_response | jq .

quoteId=$(jq -r '.id' <<< "${quote_response}")
buyAmount=$(jq -r '.quote.buyAmount' <<< "${quote_response}")
feeAmount=$(jq -r '.quote.feeAmount' <<< "${quote_response}")
validTo=$(jq -r '.quote.validTo' <<< "${quote_response}")
sellAmount=$((SELL_AMOUNT - feeAmount))

echo "Quote ID: $quoteId"
echo "Buy amount (before slippage): $buyAmount"
echo "Fee amount: $feeAmount"
echo "Sell amount (after fee): $sellAmount"
echo "Valid until: $validTo"

# Apply slippage
buyAmount=$((buyAmount * ( 100 - $SLIPPAGE ) / 100 ))
echo "Buy amount (after slippage): $buyAmount"

# Create and sign the order
echo "Creating order..."
order_payload='{
  "sellToken": "'$SELL_TOKEN'",
  "buyToken": "'$BUY_TOKEN'",
  "receiver": "'$receiver'",
  "sellAmount": "'$sellAmount'",
  "buyAmount": "'$buyAmount'",
  "validTo": '$validTo',
  "appData": "'$app_data_hash'",
  "feeAmount": "'$feeAmount'",
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "from": "'$receiver'",
  "quoteId": '$quoteId'
}'

echo "Order payload:"
echo $order_payload | jq .

# Post the order
echo "Posting order to orderbook..."
order_response=$( curl --retry 5 --fail-with-body -s --show-error -X 'POST' \
  "http://$HOST/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d "$order_payload")

echo "Order response:"
echo $order_response | jq .

orderUid=$(jq -r '.' <<< "${order_response}")

if [ "$orderUid" = "null" ] || [ -z "$orderUid" ]; then
  echo "Failed to create order"
  echo $order_response
  exit 1
fi

echo "Order UID: $orderUid"

# Wait for order to be traded
echo "Waiting for order to be traded..."
for i in $(seq 1 24);
do
  orderStatus=$( curl --retry 5 --fail-with-body -s --show-error -X 'GET' \
    "http://$HOST/api/v1/orders/$orderUid" \
    -H 'accept: application/json' | jq -r '.status')
  echo -e -n "Order status: $orderStatus     \r"
  if [ "$orderStatus" = "fulfilled" ]; then
    echo -e "\n✅ Success - Order was traded!"
    exit 0
  fi
  sleep 5
done

echo -e "\n❌ Timeout - Order was not traded within 2 minutes"
exit 1
