// price = (sqrtPriceX96 / 2^96)^2 * 10^(dec0 - dec1)  →  token1 per token0 (human units)
export function computePrice(sqrtPrice: string, dec0: number, dec1: number): string {
  try {
    const sq = BigInt(sqrtPrice)
    if (sq === 0n) return '0'
    const Q96 = 2n ** 96n
    const PREC = 10n ** 18n
    // price_scaled = sq^2 / Q96^2 * PREC  (18 decimal fixed-point)
    const scaled = (sq * sq * PREC) / (Q96 * Q96)
    const diff = dec0 - dec1
    const adjusted =
      diff >= 0 ? scaled * 10n ** BigInt(diff) : scaled / 10n ** BigInt(-diff)
    return formatFixed(adjusted, PREC, 6)
  } catch {
    return '?'
  }
}

function formatFixed(val: bigint, scale: bigint, sigFigs: number): string {
  const int = val / scale
  const frac = (val % scale).toString().padStart(String(scale).length - 1, '0')
  if (int > 0n) return `${int}.${frac.slice(0, sigFigs)}`
  const first = frac.search(/[1-9]/)
  if (first === -1) return '~0'
  return `0.${'0'.repeat(first)}${frac.slice(first, first + sigFigs)}`
}

export function formatLiquidity(val: string): string {
  try {
    const n = BigInt(val)
    const neg = n < 0n
    const abs = neg ? -n : n
    const sign = neg ? '−' : ''
    if (abs === 0n) return '0'
    const tiers: [bigint, string][] = [
      [10n ** 18n, 'e18'],
      [10n ** 15n, 'e15'],
      [10n ** 12n, 'T'],
      [10n ** 9n, 'B'],
      [10n ** 6n, 'M'],
      [10n ** 3n, 'K'],
    ]
    for (const [threshold, suffix] of tiers) {
      if (abs >= threshold) {
        const int = abs / threshold
        const frac = ((abs % threshold) * 100n / threshold).toString().padStart(2, '0')
        return `${sign}${int}.${frac}${suffix}`
      }
    }
    return `${sign}${abs}`
  } catch {
    return val
  }
}

export function short(addr: string): string {
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`
}

export function feeTierLabel(ppm: string): string {
  const labels: Record<string, string> = { '100': '0.01%', '500': '0.05%', '3000': '0.3%', '10000': '1%' }
  return labels[ppm] ?? `${(Number(ppm) / 10000).toFixed(2)}%`
}
