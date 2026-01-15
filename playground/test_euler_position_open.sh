#!/bin/bash
#set -x

# address of the CowEvcOpenPositionWrapper
# updated Jan 15
WRAPPER_CONTRACT=${WRAPPER_CONTRACT:-0xE87A2B78260c00423303D9677f42413254bbAF3c}

# length of the wrapper data in hex with 0x at the beginning
#WRAPPER_LEN=0x0120

# this wrapper data is more or less just hardcoded from some tests for speed
# information on how it is encoded can be found on the GW page
# it includes collateral vault, borrow vault, and amount to borrow in hex
# 0x797... is eUSDC
# 0xD8b... is eWETH
WRAPPER_DATA=\
000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266\
000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266\
00000000000000000000000000000000000000000000000000000000ff$(openssl rand -hex 3)\
000000000000000000000000D8b27CF359b7D15710a5BE299AF6e7Bf904984C2\
000000000000000000000000797DD80692c3b2dAdabCe8e30C07fDE5307D48a9\
0000000000000000000000000000000000000000000000000de0b6b3a7640000\
0000000000000000000000000000000000000000000000000000000005f5e100

WRAPPER_APPROVAL_HASH=$(cast call $WRAPPER_CONTRACT 0x4fedcdbf$WRAPPER_DATA)

echo "wrapper approval hash: $WRAPPER_APPROVAL_HASH"

# we use a presigned hash so the signature is just empty
SIG_DATA=\
0000000000000000000000000000000000000000000000000000000000000100\
0000000000000000000000000000000000000000000000000000000000000000

appData='{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"'${WRAPPER_CONTRACT}'\",\"data\":\"'0x${WRAPPER_DATA}${SIG_DATA}'\",\"isOmittable\":false}]}}'
appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{\"wrappers\":[{\"address\":\"${WRAPPER_CONTRACT}\",\"data\":\"0x${WRAPPER_DATA}${SIG_DATA}\",\"isOmittable\":false}]}}"
#appData='{\"version\":\"0.9.0\",\"metadata\":{}}'
#appDataUnescaped="{\"version\":\"0.9.0\",\"metadata\":{}}"

echo "computed app data: $appData"

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

echo 'approve USDC to settlement...'
cast send 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 'approve(address,uint256)' 0xD5D7ae3dD0C1c79DB7B0307e0d36AEf14eEee205 115792089237316195423570985008687907853269984665640564039457584007913129639935 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

echo 'set operator...'
cast send 0x0C9a3dd6b8F28529d72d7f9cE918D493519EE383 'setAccountOperator(address account, address operator, bool authorized)' 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 $WRAPPER_CONTRACT true --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

# Then, deposit into the eWETH vault
#echo 'convert to eWETH...'
#cast send 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'deposit(uint256,address)' 1000000000000000000 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
#sleep 0.1
#cast rpc evm_mine
#wait

# then, approve max
#echo 'approve settlement spending...'
#cast send 0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2 'approve(address,uint256)' 0xD5D7ae3dD0C1c79DB7B0307e0d36AEf14eEee205 115792089237316195423570985008687907853269984665640564039457584007913129639935 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
#sleep 0.1
#cast rpc evm_mine
#wait

# pre-sign the hash
echo 'wrapper pre-sign...'
cast send $WRAPPER_CONTRACT 'setPreApprovedHash(bytes32,bool)' $WRAPPER_APPROVAL_HASH true --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 &
sleep 0.1
cast rpc evm_mine
wait

sellAmount=100000000
#buyAmount=10000000000000
buyAmount=1

echo "ready to sell $sellAmount to at least $buyAmount"

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
    "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    "buyToken": "0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2",
    "receiver": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
    "sellAmount": "'${sellAmount}'",
    "buyAmount": "'${buyAmount}'",
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
  "sellToken": "'0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'",
  "buyToken": "'0xD8b27CF359b7D15710a5BE299AF6e7Bf904984C2'",
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
  "sellAmount": "'${sellAmount}'", 
  "buyAmount": "'${buyAmount}'", 
  "appData": "'$appData'",
  "appDataHash": "'$appDataHash'"
}'
