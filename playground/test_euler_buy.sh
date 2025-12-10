#!/bin/bash
#set -x

#WRAPPER_CONTRACT=${WRAPPER_CONTRACT:-0x751871E9cA28B441Bb6d3b7C4255cf2B5873d56a}

#appData='{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"'${WRAPPER_CONTRACT}'\",\"data\":\"0x\",\"isOmittable\":false}]}}'
#appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"${WRAPPER_CONTRACT}\",\"data\":\"0x\",\"isOmittable\":false}]}}"
appData='{\"version\":\"0.9.0\",\"metadata\":{}}'
appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{}}"


appDataHash=$(cast keccak "$appDataUnescaped")
#appDataHash="0x0000000000000000000000000000000000000000000000000000000000000000"

# valid until 24 hours from now
validTo=$(date -d "5 minutes" +%s)

# first, turn some ETH into WETH (we can just throw ETH at the WETH contract and it will auto wrap to sender)
echo 'convert to WETH...'
cast send 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 --value 1000000000000000000 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

echo 'approve eWETH deposit...'
cast send 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 'approve(address,uint256)' 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 115792089237316195423570985008687907853269984665640564039457584007913129639935 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

# Then, deposit into the eWETH vault
echo 'convert to eWETH...'
cast send 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'deposit(uint256,address)' 1000000000000000000 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

# then, approve max
echo 'approve settlement spending...'
cast send 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'approve(address,uint256)' 0xD5D7ae3dD0C1c79DB7B0307e0d36AEf14eEee205 115792089237316195423570985008687907853269984665640564039457584007913129639935 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait
sleep 0.1
echo "query balance: $(cast call 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'balanceOf(address) returns (uint256)' 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266)"

sellAmount=$(cast call 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'balanceOf(address) returns (uint256)' 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --json | jq -r '.[0]')

echo "ready to sell $sellAmount"

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
    "verifyingContract": "0x99B14b6C733a8E2196d5C561e6B5F6f083F4a7f9"
  },
  "message": {
    "sellToken": "0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2",
    "buyToken": "0x797DD80692c3b2dAdabCe8e30C07fDE5307D48a9",
    "receiver": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
    "sellAmount": "'${sellAmount}'",
    "buyAmount": "1000000000",
    "validTo": '${validTo}',
    "appData": "'${appDataHash}'",
    "feeAmount": "0",
    "kind": "buy",
    "partiallyFillable": false,
    "sellTokenBalance": "erc20",
    "buyTokenBalance": "erc20"
  }
}
'

echo 'sign...'
sig=$(cast wallet sign --data "${signData}" --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80)

# EWETH to EUSDS
echo 'order...'
set -x
curl --fail-with-body -s --show-error -X 'POST' \
  "http://localhost:8080/api/v1/orders" \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "sellToken": "'0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2'",
  "buyToken": "'0x797DD80692c3b2dAdabCe8e30C07fDE5307D48a9'",
  "from": "'0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'",
  "receiver": "'0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'",
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "onchainOrder": false,
  "partiallyFillable": false,
  "kind": "buy",
  "validTo": '${validTo}',
  "feeAmount": "0",
  "signature": "'${sig}'",
  "sellAmount": "'${sellAmount}'", 
  "buyAmount": "'1000000000'", 
  "appData": "'$appData'",
  "appDataHash": "'$appDataHash'"
}'
