#!/usr/bin/env python3
"""
Sign CoW Protocol orders using EIP-712
"""
import json
import sys
from eth_account import Account
from eth_account.messages import encode_typed_data

def create_order_typed_data(chain_id, verifying_contract, order_params):
    """Create EIP-712 typed data for CoW Protocol order"""
    domain = {
        "name": "Gnosis Protocol",
        "version": "v2",
        "chainId": chain_id,
        "verifyingContract": verifying_contract
    }
    
    types = {
        "EIP712Domain": [
            {"name": "name", "type": "string"},
            {"name": "version", "type": "string"},
            {"name": "chainId", "type": "uint256"},
            {"name": "verifyingContract", "type": "address"}
        ],
        "Order": [
            {"name": "sellToken", "type": "address"},
            {"name": "buyToken", "type": "address"},
            {"name": "receiver", "type": "address"},
            {"name": "sellAmount", "type": "uint256"},
            {"name": "buyAmount", "type": "uint256"},
            {"name": "validTo", "type": "uint32"},
            {"name": "appData", "type": "bytes32"},
            {"name": "feeAmount", "type": "uint256"},
            {"name": "kind", "type": "string"},
            {"name": "partiallyFillable", "type": "bool"},
            {"name": "sellTokenBalance", "type": "string"},
            {"name": "buyTokenBalance", "type": "string"}
        ]
    }
    
    return {
        "types": types,
        "primaryType": "Order",
        "domain": domain,
        "message": order_params
    }

def sign_order(private_key, chain_id, settlement_contract, order_params):
    """Sign a CoW Protocol order"""
    # Create typed data
    typed_data = create_order_typed_data(chain_id, settlement_contract, order_params)

    # Encode and sign
    encoded_data = encode_typed_data(full_message=typed_data)
    account = Account.from_key(private_key)
    signed_message = account.sign_message(encoded_data)

    # Create full order payload - convert numeric fields appropriately for API
    order_payload = {}
    for key, value in order_params.items():
        # Convert u256 fields to strings (orderbook expects u256 as strings)
        if key in ["sellAmount", "buyAmount", "feeAmount"] and value is not None:
            order_payload[key] = str(value)
        # validTo is u32, keep as integer
        elif key == "validTo" and value is not None:
            order_payload[key] = int(value)
        else:
            order_payload[key] = value

    order_payload["signingScheme"] = "eip712"
    order_payload["signature"] = "0x" + signed_message.signature.hex()
    order_payload["from"] = account.address

    return order_payload

def normalize_order_params(params):
    """
    Normalize order parameters - convert string 'null' to None and ensure correct types
    """
    normalized = {}
    required_numeric = ["sellAmount", "buyAmount", "validTo"]

    for key, value in params.items():
        # Handle string "null" from bash/jq
        if value == "null" or value is None:
            if key in required_numeric:
                raise ValueError(f"Required field '{key}' is null or missing. This usually means the quote API failed. Check the quote response.")
            normalized[key] = None
        # Convert numeric strings to integers for numeric fields
        elif key in ["sellAmount", "buyAmount", "feeAmount", "validTo", "chainId"]:
            try:
                normalized[key] = int(value) if value else 0
            except (ValueError, TypeError):
                if key in required_numeric:
                    raise ValueError(f"Invalid value for '{key}': {value}")
                normalized[key] = 0
        # Convert boolean strings
        elif key == "partiallyFillable":
            if isinstance(value, bool):
                normalized[key] = value
            elif isinstance(value, str):
                normalized[key] = value.lower() in ("true", "1", "yes")
            else:
                normalized[key] = bool(value)
        else:
            normalized[key] = value

    # Validate required fields
    for field in required_numeric:
        if field not in normalized or normalized[field] is None:
            raise ValueError(f"Required field '{field}' is missing from order parameters")

    return normalized

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: sign_order.py <private_key> '<order_json>'")
        sys.exit(1)

    private_key = sys.argv[1]
    order_params = json.loads(sys.argv[2])

    # Default values
    chain_id = order_params.get("chainId", 31337)
    settlement = order_params.get("settlement", "0x610178dA211FEF7D417bC0e6FeD39F05609AD788")

    # Remove metadata fields
    order_params.pop("chainId", None)
    order_params.pop("settlement", None)

    # Normalize parameters (handle "null" strings from bash)
    order_params = normalize_order_params(order_params)

    # Sign the order
    signed_order = sign_order(private_key, chain_id, settlement, order_params)
    
    # Output JSON
    print(json.dumps(signed_order, indent=2))
