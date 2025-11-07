#!/bin/bash
set -e

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# Get the project root (parent of scripts directory)
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

echo "🚀 Starting Anvil with state management..."

# Load environment variables from project root
if [ -f "$PROJECT_ROOT/.env" ]; then
    source "$PROJECT_ROOT/.env"
else
    echo "⚠️  Warning: .env file not found, using defaults"
    RPC_URL="http://localhost:8545"
fi

# Use absolute path for state file
STATE_FILE="$PROJECT_ROOT/state/poc-state.json"

# Create state directory if it doesn't exist
mkdir -p "$PROJECT_ROOT/state"

# Check if state file exists
if [ -f "$STATE_FILE" ]; then
    echo "📂 Loading existing state from $STATE_FILE"
    LOAD_STATE_FLAG="--load-state $STATE_FILE"
else
    echo "🆕 Starting fresh - no state file found"
    LOAD_STATE_FLAG=""
fi

# Start Anvil with state dumping
echo "⚡ Starting Anvil on http://localhost:8545..."
echo ""
echo "Network Configuration:"
echo "  Chain ID: 31337"
echo "  Block Time: 1 second"
echo "  Gas Limit: 30,000,000"
echo "  State File: $STATE_FILE"
echo ""
echo "Test Accounts:"
echo "  Alice:  0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
echo "  Bob:    0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
echo "  Carol:  0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
echo ""
echo "Press CTRL+C to stop (state will be saved automatically)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

anvil \
    $LOAD_STATE_FLAG \
    --dump-state "$STATE_FILE" \
    --host 0.0.0.0 \
    --port 8545 \
    --chain-id 31337 \
    --block-time 1 \
    --gas-limit 30000000 \
    --code-size-limit 50000 \
    --accounts 10 \
    --balance 10000 \
    --mnemonic "test test test test test test test test test test test junk"

# State is automatically saved when Anvil is stopped (CTRL+C)
