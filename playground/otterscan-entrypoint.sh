#!/bin/sh
# Otterscan entrypoint that configures Sourcify based on SOURCIFY_MODE

CONFIG_FILE="/usr/share/nginx/html/config.json"
ERIGON_URL="${ERIGON_URL:-http://127.0.0.1:8545}"
LOCAL_SOURCIFY_URL="${LOCAL_SOURCIFY_URL:-http://localhost:5555}"

echo "=== Otterscan Entrypoint ==="
echo "SOURCIFY_MODE: ${SOURCIFY_MODE}"

case "${SOURCIFY_MODE:-cloud}" in
  local)
    echo "Using LOCAL Sourcify as primary source"
    cat > "$CONFIG_FILE" << EOF
{
  "erigonURL": "${ERIGON_URL}",
  "sourcify": {
    "sources": {
      "Local Sourcify": {
        "url": "${LOCAL_SOURCIFY_URL}/repository",
        "backendFormat": "RepositoryV1"
      }
    }
  }
}
EOF
    ;;
  cloud|*)
    echo "Using CLOUD Sourcify as primary source"
    cat > "$CONFIG_FILE" << EOF
{
  "erigonURL": "${ERIGON_URL}",
  "sourcify": {
    "sources": {
      "Sourcify": {
        "url": "https://repo.sourcify.dev",
        "backendFormat": "RepositoryV1"
      }
    }
  }
}
EOF
    ;;
esac

echo "Config written to $CONFIG_FILE:"
cat "$CONFIG_FILE"
echo ""
echo "=== Starting nginx ==="

exec nginx -g "daemon off;"
