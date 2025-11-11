#!/usr/bin/env python3

# Event DebugCreate2(address factory, address token0, address token1, bytes32 salt, bytes32 initCodeHash, address expectedPair)
# Data from first event (WETH/USDC):
data = "0x000000000000000000000000dc64a140aa3e981100a9beca4e685f962f0cf6c90000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa30000000000000000000000009fe46736679d2d9a65f0992f2272de9f3c7fa6e04c038ad6bfca2133ee6f20ac54377d10fb6f3756d2321ead7428e24817161e77b6912aa8f91da604bdd903b3484a9f6bb569baa993085fc590133487ff27f91e0000000000000000000000005a270e627326bf3d504d7e41ccff73ca6af16227"

# Remove 0x prefix
data = data[2:]

# Each parameter is 32 bytes (64 hex chars)
factory = "0x" + data[24:64]  # address is 20 bytes, padded to 32
token0 = "0x" + data[88:128]
token1 = "0x" + data[152:192]
salt = "0x" + data[192:256]
initCodeHash = "0x" + data[256:320]
expectedPair = "0x" + data[344:384]

print(f"DEBUG: data length = {len(data)}")
print(f"DEBUG: full data = {data[:400]}")

print("=== DebugCreate2 Event Data ===")
print(f"Factory:         {factory}")
print(f"Token0 (WETH):   {token0}")
print(f"Token1 (USDC):   {token1}")
print(f"Salt:            {salt}")
print(f"Init Code Hash:  {initCodeHash}")
print(f"Expected Pair:   {expectedPair}")
print()

# Now verify the actual deployed pair address
print("Checking actual deployed pair...")
import subprocess
result = subprocess.run([
    "cast", "call", factory,
    "getPair(address,address)(address)",
    token0, token1,
    "--rpc-url", "http://127.0.0.1:8545"
], capture_output=True, text=True)

actual_pair = result.stdout.strip()
print(f"Actual Pair:     {actual_pair}")
print()
print(f"Match: {expectedPair.lower() == actual_pair.lower()}")
