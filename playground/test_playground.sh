#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
SELLTOKEN="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
BUYTOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
RECEIVER="0xa0Ee7A142d267C1f36714E4a8F75612F20a79720"
AMOUNT="1000000000000000000"
PRIVATEKEY="0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6" # cast wallet new

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
  "sellAmountBeforeFee": "'$AMOUNT'"
}')
sellAmount=$(jq -r --args '.quote.sellAmount' <<< "${quote_reponse}")
buyAmount=$(jq -r --args '.quote.buyAmount' <<< "${quote_reponse}")
feeAmount=$(jq -r --args '.quote.feeAmount' <<< "${quote_reponse}")
#echo -e $sellAmount"\n"$buyAmount"\n"$feeAmount
validTo=$(($(date +%s) + 120)) # validity time: now + 2 minutes
#order="0x"$(printf '%s' "$quote_reponse" | hexdump -ve '/1 "%02X"') #"$quote_reponse #(jq -r --args '.quote' <<< "${quote_reponse}")

echo $quote_reponse

# Filter out unneeded fields
order_proposal=$(jq -r --args '.quote|=(.appData="0xb48d38f93eaa084033fc5970bf96e559c33c4cdc07d889ab00b4d63f9590739d") | del(.quote.appDataHash) | .quote|=(.sellAmount="'$AMOUNT'") | .quote|=(.feeAmount="0") | .quote' <<< "${quote_reponse}")

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
    "name": "Ethereum Mainnet",
    "version": "1",
    "chainId": 1,
    "verifyingContract": "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
    },
  "message": '$order_proposal'
}'

echo .
echo $eip712_typed_struct
echo .

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
