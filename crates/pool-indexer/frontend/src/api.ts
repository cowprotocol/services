export interface Token {
  id: string
  decimals: number
  symbol?: string
}

export interface Pool {
  id: string
  token0: Token
  token1: Token
  fee_tier: string
  liquidity: string
  sqrt_price: string
  tick: number
  ticks: null
}

export interface PoolsResponse {
  block_number: number
  pools: Pool[]
  next_cursor: string | null
}

export interface Tick {
  tick_idx: number
  liquidity_net: string
}

export interface TicksResponse {
  block_number: number
  pool: string
  ticks: Tick[]
}

export async function fetchPools(
  network: string,
  after?: string,
  limit = 1000,
  search?: { token0: string; token1?: string },
): Promise<PoolsResponse> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (after) params.set('after', after)
  if (search) {
    params.set('token0', search.token0)
    if (search.token1) params.set('token1', search.token1)
  }
  const res = await fetch(`/api/v1/${network}/uniswap/v3/pools?${params}`)
  if (res.status === 503) throw new Error('not_indexed')
  if (!res.ok) {
    const body = await res.json().catch(() => ({})) as { error?: string }
    throw new Error(body.error ?? `HTTP ${res.status}`)
  }
  return res.json() as Promise<PoolsResponse>
}

export async function fetchTicks(network: string, poolAddress: string): Promise<TicksResponse> {
  const res = await fetch(`/api/v1/${network}/uniswap/v3/pools/${poolAddress}/ticks`)
  if (res.status === 503) throw new Error('not_indexed')
  if (!res.ok) {
    const body = await res.json().catch(() => ({})) as { error?: string }
    throw new Error(body.error ?? `HTTP ${res.status}`)
  }
  return res.json() as Promise<TicksResponse>
}
