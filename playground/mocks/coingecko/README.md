# Coingecko Mock API

Mock implementation of the Coingecko API for offline development and testing.

## Overview

This service provides a lightweight mock of the Coingecko API, specifically the `/api/v3/simple/token_price` endpoint. It returns configurable prices for the 5 tokens deployed in offline mode.

## Supported Tokens

The following tokens are supported (configured in `src/tokens.config.ts`):

| Symbol | Address | Price (ETH) |
|--------|---------|-------------|
| WETH | `0xb3af08c783c4d9c380893257980b5e26657f2317` | 1.0 |
| DAI | `0xb12812c0cad46d18b669b31059d485fe90b1a839` | 0.0004 |
| USDC | `0xb04afbcd351a0a7e4ff658b3772ee5f3f5b6e4ae` | 0.0004 |
| USDT | `0x171a30524fd943df1a12cbb9da291bf4e34ac84b` | 0.0004 |
| GNO | `0x51a53858a4a8b81814da35c4604eb9003d56a895` | 0.05 |

## API Endpoints

### Price Fetching

```bash
GET /api/v3/simple/token_price/ethereum?contract_addresses=<addresses>&vs_currencies=eth&precision=full
```

**Query Parameters:**
- `contract_addresses`: Comma-separated list of token contract addresses
- `vs_currencies`: Currency denomination (only `eth` is supported)
- `precision`: Precision level (e.g., `full`)

**Example Request:**
```bash
curl "http://localhost:3000/api/v3/simple/token_price/ethereum?contract_addresses=0xb3af08c783c4d9c380893257980b5e26657f2317,0xb12812c0cad46d18b669b31059d485fe90b1a839&vs_currencies=eth&precision=full"
```

**Example Response:**
```json
{
  "0xb3af08c783c4d9c380893257980b5e26657f2317": {
    "eth": 1.0
  },
  "0xb12812c0cad46d18b669b31059d485fe90b1a839": {
    "eth": 0.0004
  }
}
```

### Health Check

```bash
GET /health
```

Returns the service status.

### List Supported Tokens

```bash
GET /api/v3/tokens
```

Returns a list of all supported token addresses.

## Local Development

### Prerequisites

- Node.js 20+
- npm or yarn

### Installation

```bash
npm install
```

### Running

```bash
# Development mode with hot reload
npm run dev

# Production build
npm run build
npm start
```

### Environment Variables

- `PORT`: Server port (default: 3000)

## Docker

### Build

```bash
docker build -t coingecko-mock .
```

### Run

```bash
docker run -p 3000:3000 coingecko-mock
```

## Configuration

To add or modify token prices, edit `src/tokens.config.ts` and update the `TOKENS` object.

## Limitations

- Only supports Ethereum platform (`ethereum`)
- Only supports ETH denomination (`vs_currencies=eth`)
- Only returns prices for tokens defined in `tokens.config.ts`
- Does not support historical prices, market caps, or other Coingecko features
