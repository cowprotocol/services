import { useState, useEffect, useCallback } from 'react'
import { fetchPools, fetchTicks } from './api'
import type { Pool, Tick } from './api'
import { computePrice, short, feeTierLabel, formatLiquidity, explainTicks } from './utils'
import './App.css'

interface TicksState {
  data: Tick[]
  blockNumber: number
  poolId: string
}

function getNetworkFromPath(): string {
  const segment = window.location.pathname.split('/').filter(Boolean)[0]
  if (!segment) {
    window.location.replace('/mainnet')
    return 'mainnet'
  }
  return segment
}

export default function App() {
  const network = getNetworkFromPath()
  const [pools, setPools] = useState<Pool[]>([])
  const [blockNumber, setBlockNumber] = useState<number | null>(null)
  // undefined = not yet fetched, null = last page, string = next cursor
  const [cursor, setCursor] = useState<string | null | undefined>(undefined)
  const [loadingPools, setLoadingPools] = useState(false)
  const [poolsError, setPoolsError] = useState<string | null>(null)
  const [selectedPool, setSelectedPool] = useState<string | null>(null)
  const [ticksState, setTicksState] = useState<TicksState | null>(null)
  const [loadingTicks, setLoadingTicks] = useState(false)
  const [ticksError, setTicksError] = useState<string | null>(null)
  const [showExplain, setShowExplain] = useState(false)
  const [filterInput, setFilterInput] = useState('')
  const [activeFilter, setActiveFilter] = useState('')
  const [sortDesc, setSortDesc] = useState(true)

  const loadPools = useCallback(async (after?: string) => {
    setLoadingPools(true)
    setPoolsError(null)
    try {
      const res = await fetchPools(network, after)
      setPools(prev => (after ? [...prev, ...res.pools] : res.pools))
      setBlockNumber(res.block_number)
      setCursor(res.next_cursor)
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Unknown error'
      if (msg === 'not_indexed') {
        setPoolsError('Indexer starting up — no blocks indexed yet. Retrying in 5s…')
        setTimeout(() => loadPools(after), 5000)
      } else {
        setPoolsError(msg)
      }
    } finally {
      setLoadingPools(false)
    }
  }, [network])

  useEffect(() => {
    loadPools()
  }, [loadPools])

  const doSearch = useCallback(
    async (searchStr: string) => {
      const f = searchStr.trim()
      setPools([])
      setCursor(undefined)
      if (!f) {
        loadPools()
        return
      }
      setLoadingPools(true)
      setPoolsError(null)
      try {
        const parts = f.includes('/') ? f.split('/').map(s => s.trim()).filter(Boolean) : [f]
        const search =
          parts.length >= 2 ? { token0: parts[0], token1: parts[1] } : { token0: parts[0] }
        const res = await fetchPools(network, undefined, 5000, search)
        setPools(res.pools)
        setBlockNumber(res.block_number)
        setCursor(res.next_cursor)
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Unknown error'
        setPoolsError(msg)
      } finally {
        setLoadingPools(false)
      }
    },
    [network, loadPools],
  )

  const openTicks = useCallback(
    async (poolId: string) => {
      if (selectedPool === poolId) {
        setSelectedPool(null)
        setTicksState(null)
        setShowExplain(false)
        return
      }
      setSelectedPool(poolId)
      setTicksState(null)
      setTicksError(null)
      setShowExplain(false)
      setLoadingTicks(true)
      try {
        const res = await fetchTicks(network, poolId)
        setTicksState({ data: res.ticks, blockNumber: res.block_number, poolId })
      } catch (e) {
        setTicksError(e instanceof Error ? e.message : 'Unknown error')
      } finally {
        setLoadingTicks(false)
      }
    },
    [selectedPool],
  )

  const displayPools = pools.toSorted((a, b) => {
    try {
      const diff = BigInt(a.liquidity) - BigInt(b.liquidity)
      return sortDesc ? (diff < 0n ? 1 : diff > 0n ? -1 : 0) : (diff < 0n ? -1 : diff > 0n ? 1 : 0)
    } catch {
      return 0
    }
  })

  const selectedPoolData = pools.find(p => p.id === selectedPool)

  return (
    <div className="app">
      <header>
        <div className="header-left">
          <span className="logo">Uniswap V3 Pools</span>
          <span className="network-badge">{network}</span>
          {blockNumber !== null && (
            <span className="chip">block {blockNumber.toLocaleString()}</span>
          )}
        </div>
        <div className="header-right">
          {pools.length > 0 && (
            <span className="chip">{pools.length.toLocaleString()} pools</span>
          )}
          <input
            className="filter"
            placeholder="Symbol, address, or USDC/WETH — press Enter"
            value={filterInput}
            onChange={e => setFilterInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') { setActiveFilter(filterInput); doSearch(filterInput) }
              if (e.key === 'Escape') { setFilterInput(''); setActiveFilter(''); doSearch('') }
            }}
            spellCheck={false}
          />
        </div>
      </header>

      {poolsError && <div className="banner error">{poolsError}</div>}

      <div className="layout">
        <div className="pool-panel">
          <table>
            <thead>
              <tr>
                <th>Pool</th>
                <th>Token 0</th>
                <th>Token 1</th>
                <th>Fee</th>
                <th className="r">Price (T1/T0)</th>
                <th className="r sortable" onClick={() => setSortDesc(d => !d)}>
                  Liquidity {sortDesc ? '↓' : '↑'}
                </th>
                <th className="r">Tick</th>
              </tr>
            </thead>
            <tbody>
              {displayPools.map(pool => (
                <tr
                  key={pool.id}
                  className={selectedPool === pool.id ? 'selected' : undefined}
                  onClick={() => openTicks(pool.id)}
                >
                  <td className="mono" title={pool.id}>
                    {short(pool.id)}
                  </td>
                  <td className="mono" title={pool.token0.id}>
                    {pool.token0.symbol ?? short(pool.token0.id)}
                    <span className="dim"> {pool.token0.decimals}d</span>
                  </td>
                  <td className="mono" title={pool.token1.id}>
                    {pool.token1.symbol ?? short(pool.token1.id)}
                    <span className="dim"> {pool.token1.decimals}d</span>
                  </td>
                  <td>
                    <span className="fee-badge">{feeTierLabel(pool.fee_tier)}</span>
                  </td>
                  <td className="mono r">
                    {computePrice(pool.sqrt_price, pool.token0.decimals, pool.token1.decimals)}
                  </td>
                  <td className="mono r">{formatLiquidity(pool.liquidity)}</td>
                  <td className="mono r">{pool.tick.toLocaleString()}</td>
                </tr>
              ))}
              {!loadingPools && displayPools.length === 0 && (
                <tr>
                  <td colSpan={7} className="empty">
                    {activeFilter ? 'No pools match filter.' : 'No pools loaded.'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>

          {loadingPools && pools.length === 0 && (
            <div className="status-row">Loading pools…</div>
          )}
          {typeof cursor === 'string' && !loadingPools && (
            <div className="load-more-row">
              <button onClick={() => loadPools(cursor)}>Load more</button>
            </div>
          )}
          {typeof cursor === 'string' && loadingPools && pools.length > 0 && (
            <div className="status-row dim">Loading more…</div>
          )}
        </div>

        {selectedPool && (
          <aside className="ticks-panel">
            <div className="ticks-header">
              <div>
                <h2>Ticks</h2>
                <span className="mono dim addr-small">{selectedPool}</span>
                {selectedPoolData && (
                  <div className="ticks-tokens">
                    {[selectedPoolData.token0, selectedPoolData.token1].map((t, i) => (
                      <a
                        key={i}
                        className="mono dim"
                        href={`https://etherscan.io/token/${t.id}`}
                        target="_blank"
                        rel="noreferrer"
                        title={t.id}
                      >
                        {t.symbol ?? t.id}
                        <span className="dim"> {t.decimals}d</span>
                      </a>
                    ))}
                  </div>
                )}
              </div>
              <div className="ticks-header-actions">
              {ticksState && selectedPoolData && (
                <button
                  className="explain-btn"
                  onClick={() => setShowExplain(v => !v)}
                >
                  {showExplain ? 'Hide explanation' : 'Explain'}
                </button>
              )}
              <button
                className="close"
                onClick={() => {
                  setSelectedPool(null)
                  setTicksState(null)
                  setShowExplain(false)
                }}
              >
                ✕
              </button>
              </div>
            </div>

            {loadingTicks && <div className="status-row">Loading ticks…</div>}
            {ticksError && <div className="banner error">{ticksError}</div>}

            {ticksState && (
              <>
                <div className="ticks-meta dim">
                  {ticksState.data.length} active tick
                  {ticksState.data.length !== 1 ? 's' : ''} · block{' '}
                  {ticksState.blockNumber.toLocaleString()}
                </div>
                {ticksState.data.length > 0 && selectedPoolData && (
                  <TickChart ticks={ticksState.data} currentTick={selectedPoolData.tick} />
                )}
                {showExplain && selectedPoolData && (
                  <pre className="explain-box">
                    {explainTicks({
                      token0: selectedPoolData.token0.symbol ?? selectedPoolData.token0.id,
                      token1: selectedPoolData.token1.symbol ?? selectedPoolData.token1.id,
                      currentTick: selectedPoolData.tick,
                      ticks: ticksState.data,
                    })}
                  </pre>
                )}
                <div className="ticks-table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th className="r">Tick Index</th>
                        <th className="r">Liquidity Net</th>
                      </tr>
                    </thead>
                    <tbody>
                      {ticksState.data.map(t => (
                        <tr key={t.tick_idx}>
                          <td className="mono r">{t.tick_idx.toLocaleString()}</td>
                          <td
                            className={`mono r ${t.liquidity_net.startsWith('-') ? 'neg' : 'pos'}`}
                          >
                            {formatLiquidity(t.liquidity_net)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </aside>
        )}
      </div>
    </div>
  )
}

function TickChart({ ticks, currentTick }: { ticks: Tick[]; currentTick: number }) {
  const W = 560,
    H = 80,
    PX = 16,
    PY = 8
  const minTick = ticks[0].tick_idx
  const maxTick = ticks[ticks.length - 1].tick_idx
  const tickRange = maxTick - minTick || 1

  const maxAbs = ticks.reduce((m, t) => {
    try {
      const v = BigInt(t.liquidity_net)
      const a = v < 0n ? -v : v
      return a > m ? a : m
    } catch {
      return m
    }
  }, 1n)

  const toX = (tick: number) => PX + ((tick - minTick) / tickRange) * (W - 2 * PX)
  const midY = PY + (H - 2 * PY) / 2
  const currentX = Math.min(Math.max(toX(currentTick), PX), W - PX)

  return (
    <svg className="tick-chart" viewBox={`0 0 ${W} ${H}`} aria-label="Tick distribution">
      <line x1={PX} y1={midY} x2={W - PX} y2={midY} stroke="var(--border)" strokeWidth={1} />
      {ticks.map(t => {
        const tx = toX(t.tick_idx)
        let ratio = 0
        try {
          const v = BigInt(t.liquidity_net)
          ratio = Number((v * 1000n) / maxAbs) / 1000
        } catch {
          /* skip */
        }
        const barH = Math.abs(ratio) * ((H - 2 * PY) / 2 - 2)
        const positive = ratio >= 0
        return (
          <rect
            key={t.tick_idx}
            x={tx - 1}
            y={positive ? midY - barH : midY}
            width={2}
            height={Math.max(barH, 1)}
            fill={positive ? 'var(--green)' : 'var(--red)'}
            opacity={0.8}
          />
        )
      })}
      <line
        x1={currentX}
        y1={PY}
        x2={currentX}
        y2={H - PY}
        stroke="var(--yellow)"
        strokeWidth={1.5}
        strokeDasharray="3,2"
      />
      <text x={currentX + 3} y={PY + 9} fill="var(--yellow)" fontSize={9} fontFamily="monospace">
        ▶ {currentTick}
      </text>
    </svg>
  )
}
