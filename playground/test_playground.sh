#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
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
echo "Waiting until all services are ready"
curl --retry 24 --retry-delay 5 --retry-all-errors --fail-with-body -s --show-error \
  -H 'accept:application/json' \
  http://$HOST/api/v1/token/$BUY_TOKEN/native_price > /dev/null

# Run test flow
echo "Using private key:" $PRIVATE_KEY
receiver=$(cast wallet address $PRIVATE_KEY)

# Calculate AppData hash
app_data_hash=$(cast keccak $APPDATA)

echo "Request price quote for buying USDC for WETH"
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
echo "Order UID: $orderUid"

docker exec playground-chain-1 cast send \
    --json \
    --private-key "$PRIVATE_KEY"  \
    --value "$SELL_AMOUNT" \
    "$COW_ETHFLOW_CONTRACT" \
    "createOrder((address, address, uint256, uint256, bytes32, uint256, uint32, bool, int64))" \
    "($BUY_TOKEN,$receiver,$sellAmount,$buyAmount,$app_data_hash,$feeAmount,$validTo,false,$quoteId)" > /dev/null

for i in $(seq 1 24);
do
  orderStatus=$( curl --retry 5 --fail-with-body -s --show-error -X 'GET' \
    "http://$HOST/api/v1/orders/$orderUid/status" \
    -H 'accept: application/json' | jq -r '.type')
  echo -e -n "Order status: $orderStatus     \r"
  if [ "$orderStatus" = "traded" ]; then
    echo -e "\nSuccess"
    exit 0
  fi
  sleep 5
done

echo -e "\nTimeout"
exit 1
