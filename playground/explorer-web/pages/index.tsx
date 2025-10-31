import Link from 'next/link';
import { useEffect, useState } from 'react';
import useSWR from 'swr';

export default function Home() {
  const [q, setQ] = useState('');
  const base = process.env.NEXT_PUBLIC_API_BASE || 'http://localhost:8081';
  const fetcher = (u: string) => fetch(u).then(r => r.json());
  const { data: blocks } = useSWR(`${base}/api/blocks?limit=10`, fetcher, { refreshInterval: 2000 });
  const { data: txs } = useSWR(`${base}/api/tx?limit=10`, fetcher, { refreshInterval: 2000 });

  async function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    const res = await fetch(`${base}/api/search?q=${encodeURIComponent(q)}`);
    const data = await res.json();

    // Use redirect field if present (preferred)
    if (data.redirect) {
      window.location.href = data.redirect;
      return;
    }

    // Fallback to type-based navigation
    if (data.type === 'tx') {
      window.location.href = `/tx/${data.value.tx.hash}`;
    } else if (data.type === 'block') {
      const n = parseInt(data.value.number, 16);
      window.location.href = `/block/${n}`;
    } else if (data.type === 'address') {
      window.location.href = `/address/${data.value.address}`;
    } else if (data.type === 'unknown') {
      alert(`No results found for: ${q}`);
    }
  }

  return (
    <main style={{ maxWidth: 1100, margin: '40px auto', fontFamily: 'Inter, system-ui, Arial' }}>
      <h1>Playground Explorer</h1>
      <form onSubmit={handleSearch} style={{ marginTop: 20 }}>
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search by tx hash, block number, or address"
          style={{ width: '100%', padding: 12, fontSize: 16 }}
        />
      </form>
      <div style={{ display: 'flex', gap: 24, marginTop: 30 }}>
        <div style={{ flex: 1 }}>
          <h3>Latest Blocks</h3>
          <ul>
            {blocks?.map((b: any) => (
              <li key={b.number}>
                <Link href={`/block/${b.number}`}>#{b.number}</Link> — txs: {b.txCount}
              </li>
            ))}
          </ul>
        </div>
        <div style={{ flex: 2 }}>
          <h3>Latest Transactions</h3>
          <ul>
            {txs?.map((t: any) => (
              <li key={t.hash}>
                <Link href={`/tx/${t.hash}`}>{t.hash.slice(0, 10)}...</Link>
                {' '}blk {t.blockNumber} — from {t.fromAddr?.slice(0, 8)}… to {t.toAddr?.slice(0, 8)}…
              </li>
            ))}
          </ul>
        </div>
      </div>
    </main>
  );
}
