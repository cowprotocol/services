#!/bin/bash

# Fail on all errors
set -e

# Setup parameters
export HOST=localhost:8080
export SELLTOKEN="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
export BUYTOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
export RECEIVER="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
export AMOUNT="1000000000000000000"

# Run test flow

echo "Request price qoute for buying USDC for WETH"
curl --fail-with-body -X 'POST' \
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
}' 
