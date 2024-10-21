#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
SELLTOKEN="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
BUYTOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
RECEIVER="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
AMOUNT="1000000000000000000"

# Run test flow

echo "Request price qoute for buying USDC for WETH"
quote_reponse=$( curl --fail-with-body -s -X 'POST' \
  "http://$HOST/api/v1/quote" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'$SELLTOKEN'",
  "buyToken": "'$BUYTOKEN'",
  "from": "'$RECEIVER'",
  "receiver": "'$RECEIVER'",
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "priceQuality": "fast",
  "signingScheme": "eip712",
  "onchainOrder": false,
  "partiallyFillable": false,
  "kind": "sell",
  "sellAmountBeforeFee": "'$AMOUNT'",
  "appData": "{\"version\":\"1.3.0\",\"metadata\":{}}",
  "appDataHash": "0xa872cd1c41362821123e195e2dc6a3f19502a451e1fb2a1f861131526e98fdc7"
}')
sellAmount=$(jq -r --args '.quote.sellAmount' <<< "${quote_reponse}")
buyAmount=$(jq -r --args '.quote.buyAmount' <<< "${quote_reponse}")
feeAmount=$(jq -r --args '.quote.feeAmount' <<< "${quote_reponse}")
#echo -e $sellAmount"\n"$buyAmount"\n"$feeAmount
validTo=$(($(date +%s) + 120)) # validity time: now + 2 minutes

echo "Submit an order"
orderUid=$( curl --fail-with-body -s -X 'POST' \
  "http://$HOST/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'$SELLTOKEN'",
  "buyToken": "'$BUYTOKEN'",
  "receiver": "'$RECEIVER'",
  "sellAmount": "'$sellAmount'",
  "buyAmount": "'$buyAmount'",
  "validTo": '$validTo',
  "feeAmount": "0",
  "kind": "buy",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "presign",
  "signature": "0x",
  "from": "'$RECEIVER'",
  "appData": "{\"version\":\"1.3.0\",\"metadata\":{}}",
  "appDataHash": "0xa872cd1c41362821123e195e2dc6a3f19502a451e1fb2a1f861131526e98fdc7"
}')
orderUid=${orderUid:1:-1} # remove quotes
echo "Order UID: $orderUid"

for i in $(seq 1 60);
do
  orderStatus=$( curl --fail-with-body -s -X 'GET' \
    "http://$HOST/api/v1/orders/$orderUid/status" \
    -H 'accept: application/json' | jq -r '.type')
  echo "Order status: $orderStatus"
  sleep 2
done
