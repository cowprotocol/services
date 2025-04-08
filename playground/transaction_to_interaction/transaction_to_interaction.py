# /// script
# dependencies = [
#   "web3",
#   "hexbytes",
#   "eth_typing",
#   "python-dotenv"
# ]
# ///

# script for converting uniswap router transaction into cow protocol interactions
# it can be run (after installing uv) using the command
# `uv run --script transaction_to_interaction.py`

import json
import os
import re
import subprocess

from dotenv import load_dotenv
from eth_typing import Address
from hexbytes import HexBytes
from web3 import Web3

load_dotenv()

# input
sell_token = "0x7b79995e5f793a07bc00c21412e50ecae098e7f9"  # weth
buy_token = "0x6f14C02Fc1F78322cFd7d707aB90f18baD3B54f5"
sell_amount = 10**18
buy_amount = 1

# node
# change this to use ETH_RPC_URL env variable
node_url = os.getenv("JSON_RPC_PROVIDER")
w3 = Web3(Web3.HTTPProvider(node_url))

# approve sending funds from trampoline contract
permit2_address = "0x000000000022D473030F116dDEE9F6B43aC78BA3"
permit2_approve_abi = [
    {
        "inputs": [
            {"internalType": "address", "name": "token", "type": "address"},
            {"internalType": "address", "name": "spender", "type": "address"},
            {"internalType": "uint160", "name": "amount", "type": "uint160"},
            {"internalType": "uint48", "name": "expiration", "type": "uint48"},
        ],
        "name": "approve",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function",
    },
]

permit2_contract = w3.eth.contract(
    address=Address(HexBytes(permit2_address)), abi=permit2_approve_abi
)

router_address = "0x66a9893cc07d91d95644aedd05d03f95e1dba8af"
deadline = 2**47

permit2_call_data = permit2_contract.encode_abi(
    "approve",
    args=[
        Web3.to_checksum_address(sell_token),
        Web3.to_checksum_address(router_address),
        sell_amount,
        deadline,
    ],
)

# print("permit call data:")
# print(permit2_call_data)

# router call data
ROUTER_BINARY = os.getenv("ROUTER_BINARY")

# print("get routing ...")
result = subprocess.run(
    [
        ROUTER_BINARY,
        "quote",
        "--tokenIn",
        sell_token,
        "--tokenOut",
        buy_token,
        "--amount",
        str(sell_amount / 10**18),
        "--exactIn",
        "--recipient",
        "0x741aa7cfb2c7bf2a1e7d4da2e3df6a56ca4131f3",
        "--protocols",
        "v2,v3",
        "--chainId",
        "11155111",
    ],
    capture_output=True,
    text=True,
)

# print(result.stdout)


def extract_calldata(input_string):
    # Using regular expression for start and end
    start_pattern = r"Calldata: "
    start_match = re.search(start_pattern, input_string)
    start_index = start_match.span()[1]
    end_pattern = r"Value: "
    end_match = re.search(end_pattern, input_string)
    end_index = end_match.span()[0] - 11 # shift to account for new line and stuff

    call_data = input_string[start_index:end_index]
    return call_data


router_call_data = extract_calldata(result.stdout)

# trampoline_contract_address = "0x01DCB88678AEDD0C4CC9552B20F4718550250574"
# trampoline_execute_abi = [
#     {
#         "inputs": [
#             {
#                 "components": [
#                     {"internalType": "address", "name": "target", "type": "address"},
#                     {"internalType": "bytes", "name": "callData", "type": "bytes"},
#                     {"internalType": "uint256", "name": "gasLimit", "type": "uint256"},
#                 ],
#                 "internalType": "struct HooksTrampoline.Hook[]",
#                 "name": "hooks",
#                 "type": "tuple[]",
#             }
#         ],
#         "name": "execute",
#         "outputs": [],
#         "stateMutability": "nonpayable",
#         "type": "function",
#     },
# ]

# trampoline_contract = w3.eth.contract(
#     address=Address(HexBytes(trampoline_contract_address)), abi=trampoline_execute_abi
# )

# settlement_contract = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"

# gas_limit = 10**18
# hook_call_data = trampoline_contract.encode_abi(
#     "execute",
#     args=[[
#         (
#             Web3.to_checksum_address(permit2_address),
#             permit2_call_data,
#             gas_limit,
#         )
#     ]]
# )


# print(hook_call_data)

j = json.dumps(
    [
        {"target": permit2_address, "callData": permit2_call_data, "value": "0"},
        {"target": router_address, "callData": router_call_data, "value": "0"},
    ]
)

print(j)
