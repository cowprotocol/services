#!/bin/bash
set -x

WRAPPER_CONTRACT=${WRAPPER_CONTRACT:-0x54112E2F481AC239661914691082039d7B05A264}

appData='{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"'${WRAPPER_CONTRACT}'\",\"data\":\"0x\"}]}}'
appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"${WRAPPER_CONTRACT}\",\"data\":\"0x\"}]}}"
#appData='{\"version\":\"0.9.0\",\"metadata\":{}}'
#appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{}}"


appDataHash=$(cast keccak "$appDataUnescaped")
#appDataHash="0x0000000000000000000000000000000000000000000000000000000000000000"

# valid until 24 hours from now
validTo=$(date -d "5 minutes" +%s)

# first, turn some ETH into WETH (we can just throw ETH at the WETH contract and it will auto wrap to sender)
cast send 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 --value 1000000000000000000 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# then, approve max
cast send 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 'approve(address,uint256)' 0xC92E8bdf79f0507f65a392b0ab4667716BFE0110 115792089237316195423570985008687907853269984665640564039457584007913129639935 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

signData='
{
  "types": {
    "EIP712Domain": [
      {
        "name": "name",
        "type": "string"
      },
      {
        "name": "version",
        "type": "string"
      },
      {
        "name": "chainId",
        "type": "uint256"
      },
      {
        "name": "verifyingContract",
        "type": "address"
      }
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
  "message": {
    "sellToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    "buyToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    "receiver": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
    "sellAmount": "1000000000000000000",
    "buyAmount": "1",
    "validTo": '${validTo}',
    "appData": "'${appDataHash}'",
    "feeAmount": "0",
    "kind": "sell",
    "partiallyFillable": false,
    "sellTokenBalance": "erc20",
    "buyTokenBalance": "erc20"
  }
}
'

sig=$(cast wallet sign --data "${signData}" --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80)

curl --fail-with-body -s --show-error -X 'POST' \
  "http://localhost:8080/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2'",
  "buyToken": "'0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'",
  "from": "'0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'",
  "receiver": "'0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'",
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "onchainOrder": false,
  "partiallyFillable": false,
  "kind": "sell",
  "validTo": '${validTo}',
  "feeAmount": "0",
  "signature": "'${sig}'",
  "sellAmount": "'1000000000000000000'", 
  "buyAmount": "'1'", 
  "appData": "'$appData'",
  "appDataHash": "'$appDataHash'"
}'
