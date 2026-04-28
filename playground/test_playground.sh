#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
OTTERSCAN_URL=http://localhost:8003
WETH_ADDRESS="0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"  # WETH token
SELL_TOKEN=$WETH_ADDRESS
BUY_TOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"  # USDC token
SELL_AMOUNT="1000000000000000000"  # 1 ETH
SLIPPAGE=2  # 2%
COW_ETHFLOW_CONTRACT="0x04501b9b1d52e67f6862d157e00d13419d2d6e95"
APPDATA='{"version":"1.3.0","metadata":{}}'

# Following private key is only used for testing purposes in a local environment.
# For security reasons please do not use it on a production network, most likely
# all funds sent to this account will be stolen immediately.
PRIVATE_KEY="0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"

# Wait for 2 minutes for all services are read
echo -e "\n    Waiting until all services are ready..."
curl --retry 24 --retry-delay 5 --retry-all-errors --fail-with-body -s --show-error \
  -H 'accept:application/json' \
  http://$HOST/api/v1/token/$BUY_TOKEN/native_price > /dev/null

# Run test flow
echo -e "\n    Private key: $PRIVATE_KEY"
receiver=$(cast wallet address $PRIVATE_KEY)

# Calculate AppData hash
app_data_hash=$(cast keccak $APPDATA)

echo -e "\n>>> Requesting price quote for buying USDC for WETH..."
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

quoteId=$(jq -r --args '.id' <<< "${quote_response}")
buyAmount=$(jq -r --args '.quote.buyAmount' <<< "${quote_response}")
feeAmount=0
validTo=$(($(date +%s) + 120)) # validity time: now + 2 minutes
sellAmount=$((SELL_AMOUNT - feeAmount))

# Apply slippage
buyAmount=$((buyAmount * ( 100 - $SLIPPAGE ) / 100 )) # apply slippage

# `cast call` to get the orderHash
orderHash=$(docker exec playground-chain-1 cast call \
    --json \
    --private-key "$PRIVATE_KEY"  \
    --value "$SELL_AMOUNT" \
    "$COW_ETHFLOW_CONTRACT" \
    "createOrder((address, address, uint256, uint256, bytes32, uint256, uint32, bool, int64))" \
    "($BUY_TOKEN,$receiver,$sellAmount,$buyAmount,$app_data_hash,$feeAmount,$validTo,false,$quoteId)"
)
# Encode the data to generate an orderUid
# (based on https://github.com/cowprotocol/contracts/blob/39d7f4d68e37d14adeaf3c0caca30ea5c1a2fad9/src/ts/order.ts#L302-L312)
orderUid=$(docker exec playground-chain-1 cast abi-encode \
    --packed "(bytes32,address,uint32)" \
    "$orderHash" \
    "$COW_ETHFLOW_CONTRACT" \
    "0xffffffff"
)
echo "    Order UID: $orderUid"
echo

echo -e ">>> Creating order on-chain...\n"
docker exec playground-chain-1 cast send \
    --json \
    --private-key "$PRIVATE_KEY"  \
    --value "$SELL_AMOUNT" \
    "$COW_ETHFLOW_CONTRACT" \
    "createOrder((address, address, uint256, uint256, bytes32, uint256, uint32, bool, int64))" \
    "($BUY_TOKEN,$receiver,$sellAmount,$buyAmount,$app_data_hash,$feeAmount,$validTo,false,$quoteId)" > /dev/null

print_settlement_tx() {
  trade_response=$(curl --retry 5 --fail-with-body -s --show-error -X 'GET' \
    "http://$HOST/api/v1/trades?orderUid=$orderUid" \
    -H 'accept: application/json')
  tx_hash=$(jq -r '.[0].txHash // empty' <<< "${trade_response}")

  if [ -n "$tx_hash" ]; then
    echo -e "\n--------------------------------------------------------------- SUCCESS ----------------------------------------------------------------"
    echo "    Settlement tx hash: $tx_hash"
    echo "    Inspect with: cast receipt $tx_hash --rpc-url http://localhost:8545"
    echo "    Open in Otterscan: $OTTERSCAN_URL/tx/$tx_hash"
    echo "----------------------------------------------------------------------------------------------------------------------------------------"
  else
    echo "Settlement tx hash not available yet"
  fi
}

echo ">>> Polling order status..."
for i in $(seq 1 24);
do
  status_response=$(curl --retry 5 --retry-delay 2 --retry-all-errors -s --show-error --max-time 10 --connect-timeout 3 \
    -H 'accept: application/json' \
    -w '\n%{http_code}' \
    "http://$HOST/api/v1/orders/$orderUid/status") || {
      echo "Polling failed while checking order status"
      exit 1
    }
  status_http_code=$(tail -n 1 <<< "${status_response}")
  status_body=$(sed '$d' <<< "${status_response}")

  if [ "$status_http_code" = "404" ]; then
    orderStatus="indexing"
  elif [ "$status_http_code" = "200" ]; then
    orderStatus=$(jq -r '.type' <<< "${status_body}")
  else
    echo "Unexpected order status response ($status_http_code): ${status_body}"
    exit 1
  fi

  echo "    Order status: $orderStatus"
  if [ "$orderStatus" = "traded" ]; then
    print_settlement_tx
    exit 0
  fi
  sleep 5
done

echo -e "\nTimeout"
exit 1
