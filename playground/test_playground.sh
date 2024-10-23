#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
SELLTOKEN="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
BUYTOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
RECEIVER="0x94766c15b0862Dd15F9f884D85aC1AAd34199a5f"
AMOUNT="1000000000000000000"
PRIVATEKEY="0x93de76e801fcc65f0f517c3ca716bfc49a83a922ede9d770dd788e9e47d14f60" # cast wallet new

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
#order="0x"$(printf '%s' "$quote_reponse" | hexdump -ve '/1 "%02X"') #"$quote_reponse #(jq -r --args '.quote' <<< "${quote_reponse}")

echo $quote_reponse

# Filter out unneeded fields
order_proposal=$(jq -r --args '.quote|=(.appData=.appDataHash) | del(.quote.appDataHash, .quote.signingScheme) | .quote' <<< "${quote_reponse}")

# Prepare EIP-712 typed struct
eip712_typed_struct='{
  "types": {
    "EIP712Domain": [
      { "name": "name", "type": "string" },
      { "name": "version", "type": "string" },
      { "name": "chainId", "type": "uint256" },
      { "name": "verifyingContract", "type": "address" }
    ],
    "Order": [
      { "name": "sellToken", "type": "address" },
      { "name": "buyToken", "type": "address" },
      { "name": "receiver", "type": "address" },
      { "name": "sellAmount", "type": "uint256" },
      { "name": "buyAmount", "type": "uint256" },
      { "name": "validTo", "type": "uint32" },
      { "name": "appData", "type": "bytes32" },
      { "name": "feeAmount", "type": "uint256" },
      { "name": "kind", "type": "string" },
      { "name": "partiallyFillable", "type": "bool" },
      { "name": "sellTokenBalance", "type": "string" },
      { "name": "buyTokenBalance", "type": "string" }
      ]
    },
  "primaryType": "Order",
  "domain": {
    "name": "Gnosis Protocol",
    "version": "v2",
    "chainId": 100,
    "verifyingContract": "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
    },
  "message": '$order_proposal'
}'

# Validate if json is well formatted and compact it
eip712_typed_struct=$(jq -r -c <<< "${eip712_typed_struct}")

# Dump to file as there are some spaces in field values
echo $eip712_typed_struct > tmp.json

# sign quote_reponse with private key
signature=$(cast wallet sign --private-key $PRIVATEKEY --data --from-file tmp.json)
echo "Intent signature:" $signature

echo "Submit an order"
orderUid=$( curl -v -X 'POST' \
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
  "feeAmount": "'$feeAmount'",
  "kind": "buy",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "signature": "'$signature'",
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
