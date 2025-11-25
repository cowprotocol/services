/**
 * Token configuration for Coingecko mock API
 *
 * Prices are denominated in ETH as per Coingecko's API format.
 * These are the 5 tokens deployed in offline mode with deterministic addresses.
 */

export interface TokenConfig {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  priceInEth: number;
}

export const TOKENS: Record<string, TokenConfig> = {
  // WETH - Wrapped Ether
  '0xb3af08c783c4d9c380893257980b5e26657f2317': {
    address: '0xb3af08c783c4d9c380893257980b5e26657f2317',
    symbol: 'WETH',
    name: 'Wrapped Ether',
    decimals: 18,
    priceInEth: 1.0, // 1 WETH = 1 ETH by definition
  },

  // DAI - Dai Stablecoin
  '0xb12812c0cad46d18b669b31059d485fe90b1a839': {
    address: '0xb12812c0cad46d18b669b31059d485fe90b1a839',
    symbol: 'DAI',
    name: 'Dai Stablecoin',
    decimals: 18,
    priceInEth: 0.0004, // Assuming 1 DAI ≈ $1 and 1 ETH ≈ $2500
  },

  // USDC - USD Coin
  '0xb04afbcd351a0a7e4ff658b3772ee5f3f5b6e4ae': {
    address: '0xb04afbcd351a0a7e4ff658b3772ee5f3f5b6e4ae',
    symbol: 'USDC',
    name: 'USD Coin',
    decimals: 6,
    priceInEth: 0.0004, // Assuming 1 USDC ≈ $1 and 1 ETH ≈ $2500
  },

  // USDT - Tether USD
  '0x171a30524fd943df1a12cbb9da291bf4e34ac84b': {
    address: '0x171a30524fd943df1a12cbb9da291bf4e34ac84b',
    symbol: 'USDT',
    name: 'Tether USD',
    decimals: 6,
    priceInEth: 0.0004, // Assuming 1 USDT ≈ $1 and 1 ETH ≈ $2500
  },

  // GNO - Gnosis Token
  '0x51a53858a4a8b81814da35c4604eb9003d56a895': {
    address: '0x51a53858a4a8b81814da35c4604eb9003d56a895',
    symbol: 'GNO',
    name: 'Gnosis Token',
    decimals: 18,
    priceInEth: 0.05, // Assuming 1 GNO ≈ $125 and 1 ETH ≈ $2500
  },
};

/**
 * Get token price by address (case-insensitive)
 */
export function getTokenPrice(address: string): number | null {
  const normalizedAddress = address.toLowerCase();
  const token = TOKENS[normalizedAddress];
  return token ? token.priceInEth : null;
}

/**
 * Check if token is supported
 */
export function isTokenSupported(address: string): boolean {
  return address.toLowerCase() in TOKENS;
}

/**
 * Get all supported token addresses
 */
export function getSupportedTokens(): string[] {
  return Object.keys(TOKENS);
}
