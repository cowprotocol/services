# Mock Services

This directory contains mock services used for offline development and testing of the CoW Protocol playground.

## Available Mocks

### Coingecko (`./coingecko/`)

Mock implementation of the Coingecko API for price fetching in offline mode.

- **Technology**: Node.js + TypeScript + Hono API
- **Endpoint**: `/api/v3/simple/token_price/ethereum`
- **Purpose**: Provides configurable token prices in ETH denomination without requiring external API access
- **Configuration**: Token prices can be configured in `./coingecko/src/tokens.config.ts`

**Supported Tokens** (offline mode):
- WETH
- DAI
- USDC
- USDT
- GNO

See `./coingecko/README.md` for detailed documentation.

## Architecture

Mock services are organized at the playground level to:
- Maintain clear separation from deployment infrastructure (`offline-mode/`)
- Improve discoverability of available mocks
- Enable easy addition of new mock services (e.g., block explorer, subgraph, etc.)
- Support reusability across different playground configurations

## Adding New Mocks

To add a new mock service:

1. Create a new directory: `mocks/<service-name>/`
2. Implement the mock service with a Dockerfile
3. Add the service to `docker-compose.offline.yml`
4. Update this README with documentation

## Usage

Mock services are automatically started when running the offline playground:

```bash
docker-compose -f docker-compose.offline.yml up
```

Individual mocks can be built and run separately:

```bash
docker-compose -f docker-compose.offline.yml up coingecko-mock
```
