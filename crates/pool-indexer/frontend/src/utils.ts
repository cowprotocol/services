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

function tickToPrice(tick: number): number {
  return Math.pow(1.0001, tick)
}

function inferComposition(currentTick: number, lowerTick: number, upperTick: number): string {
  if (currentTick < lowerTick) {
    return 'entirely token0 (price is below range, position is out of range)'
  } else if (currentTick >= upperTick) {
    return 'entirely token1 (price is above range, position is out of range)'
  } else {
    const progress = (currentTick - lowerTick) / (upperTick - lowerTick)
    if (progress < 0.2) return 'mostly token0 (price near lower bound)'
    if (progress > 0.8) return 'mostly token1 (price near upper bound)'
    return 'roughly balanced between token0 and token1'
  }
}

export function explainTicks({
  token0,
  token1,
  currentTick,
  ticks,
}: {
  token0: string
  token1: string
  currentTick: number
  ticks: Array<{ tick_idx: number; liquidity_net: string }>
}): string {
  // Pair ticks into LP positions: lower tick has +X liquidity net, upper has -X
  const unmatched: Record<string, { tick_idx: number; liquidity_net: string }> = {}
  const positions: Array<{
    lower: { tick_idx: number; liquidity_net: string }
    upper: { tick_idx: number; liquidity_net: string }
  }> = []

  for (const tick of ticks) {
    let net: bigint
    try {
      net = BigInt(tick.liquidity_net)
    } catch {
      continue
    }
    const absKey = net < 0n ? (-net).toString() : net.toString()
    if (unmatched[absKey]) {
      const match = unmatched[absKey]
      const lower = net > 0n ? tick : match
      const upper = net > 0n ? match : tick
      positions.push({ lower, upper })
      delete unmatched[absKey]
    } else {
      unmatched[absKey] = tick
    }
  }

  const unmatchedList = Object.values(unmatched)
  const currentPrice = tickToPrice(currentTick)
  const lines: string[] = []

  lines.push(`Pool: ${token0}/${token1}`)
  lines.push(`Current tick: ${currentTick} → 1 ${token0} = ${currentPrice.toFixed(6)} ${token1}`)
  lines.push(`Active ticks: ${ticks.length} → ${positions.length} position(s) detected`)
  lines.push('')

  positions.forEach((pos, i) => {
    const lowerPrice = tickToPrice(pos.lower.tick_idx)
    const upperPrice = tickToPrice(pos.upper.tick_idx)
    const composition = inferComposition(currentTick, pos.lower.tick_idx, pos.upper.tick_idx)
    const inRange = currentTick >= pos.lower.tick_idx && currentTick < pos.upper.tick_idx

    lines.push(`Position ${i + 1}:`)
    lines.push(`  Range: tick ${pos.lower.tick_idx} to ${pos.upper.tick_idx}`)
    lines.push(`  Price range: ${lowerPrice.toFixed(6)} to ${upperPrice.toFixed(6)} ${token1} per ${token0}`)
    lines.push(`  Status: ${inRange ? 'In range (earning fees)' : 'Out of range (not earning fees)'}`)
    lines.push(`  Composition: ${composition}`)
    lines.push('')
  })

  if (unmatchedList.length > 0) {
    lines.push(`${unmatchedList.length} unmatched tick(s) — may indicate partial data or a complex position.`)
  }

  return lines.join('\n')
}
