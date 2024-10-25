#!/bin/bash

# Fail on all errors
set -e
# Fail on expand of unset variables
set -u

# Setup parameters
HOST=localhost:8080
WETHADDRESS="0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" # WETH token
SELLTOKEN=$WETHADDRESS
BUYTOKEN="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" # USDC token
RECEIVER="0xa0Ee7A142d267C1f36714E4a8F75612F20a79720"
AMOUNT="1000000000000000000"
PRIVATEKEY="0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"
APPDATA='{"version":"1.3.0","metadata":{}}'

# Run test flow
echo "Using private key:" $PRIVATEKEY

# Calculate AppData hash
app_data_hash=$(cast keccak $APPDATA)

# Deposit WETH
echo "Wrapping some ETH"
cast send --private-key $PRIVATEKEY --value 3ether $WETHADDRESS > /dev/null

echo "Request price qoute for buying USDC for WETH"
quote_response=$( curl --fail-with-body -s -X 'POST' \
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

buyAmount=$(jq -r --args '.quote.buyAmount' <<< "${quote_response}")
feeAmount=$(jq -r --args '.quote.feeAmount' <<< "${quote_response}")
validTo=$(($(date +%s) + 120)) # validity time: now + 2 minutes
sellAmount=$((AMOUNT-feeAmount))

# Prepare EIP712 message
eip712_message=$(jq -r --args '
  .quote|=(.appData="'$app_data_hash'") | 
  del(.quote.appDataHash) | 
  .quote|=(.sellAmount="'$sellAmount'") |
  .quote|=(.feeAmount="0") |
  .quote|=(.validTo='$validTo') |
  .quote' <<< "${quote_response}")

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
    "chainId": 1,
    "verifyingContract": "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
    },
  "message": '$eip712_message'
}'

# Check if json is well formatted and compact it
eip712_typed_struct=$(jq -r -c <<< "${eip712_typed_struct}")

# Dump to file as there are some spaces in field values
echo $eip712_typed_struct > tmp.json

# Sign quote_response with private key
signature=$(cast wallet sign --private-key $PRIVATEKEY --data --from-file tmp.json)
echo "Intent signature:" $signature

app_data=${APPDATA//\"/\\\"}   # escape quotes for json field

# Update EIP712 message with additional fields required for order submit
order_proposal=$(jq -r -c --args '
  .from="'$RECEIVER'" |
  .appData|="'$app_data'" |
  .appDataHash="'$app_data_hash'" |
  .signature="'$signature'"' <<< "${eip712_message}")

echo "Submit an order"
orderUid=$( curl -v -X 'POST' \
  "http://$HOST/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d "${order_proposal}")
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
