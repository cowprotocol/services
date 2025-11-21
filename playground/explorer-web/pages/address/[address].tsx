import { useRouter } from 'next/router';
import useSWR from 'swr';

export default function AddressPage() {
  const router = useRouter();
  const { address } = router.query as { address?: string };
  const base = process.env.NEXT_PUBLIC_API_BASE || 'http://localhost:8081';
  const url = address ? `${base}/api/address/${address}` : null;
  const fetcher = (u: string) => fetch(u).then((r) => r.json());
  const { data } = useSWR(url, fetcher);
  const txUrl = address ? `${base}/api/address/${address}/txs?limit=50` : null;
  const { data: txs } = useSWR(txUrl, fetcher, { refreshInterval: 3000 });
  const abiUrl = address ? `${base}/api/abi/${address}` : null;
  const sourceUrl = address ? `${base}/api/source/${address}` : null;
  const { data: abi } = useSWR(abiUrl, fetcher);
  const { data: source } = useSWR(sourceUrl, fetcher);
  if (!address) return null;
  return (
    <main style={{ maxWidth: 900, margin: '40px auto', fontFamily: 'Inter, system-ui, Arial' }}>
      <h2>Address</h2>
      <p>{address}</p>
      {data && (
        <>
          <p>Is Contract: {String(data.isContract)}</p>
          <p>Tx Count (indexed): {data.txCount}</p>
          <p>Last Seen Block: {data.lastSeenBlock ?? 'N/A'}</p>
        </>
      )}
      <div style={{ marginTop: 16 }}>
        <h3>Verification</h3>
        {source?.verified ? (
          <>
            <p>Verified: {source.type}</p>
            <details>
              <summary>ABI</summary>
              <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>{abi ? JSON.stringify(abi, null, 2) : 'Loading...'}</pre>
            </details>
            <details>
              <summary>Sources</summary>
              <ul>
                {source?.sources?.map((s: any) => (
                  <li key={s.path}>
                    <strong>{s.path}</strong>
                    <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>{s.content}</pre>
                  </li>
                ))}
              </ul>
            </details>
          </>
        ) : (
          <p>Not verified in local Sourcify yet.</p>
        )}
      </div>
      <h3>Recent Transactions</h3>
      <ul>
        {txs?.map((t: any) => (
          <li key={t.hash}>
            <a href={`/tx/${t.hash}`}>{t.hash.slice(0, 10)}...</a> blk {t.blockNumber} — from {t.fromAddr?.slice(0,8)}… to {t.toAddr?.slice(0,8)}…
          </li>
        ))}
      </ul>
    </main>
  );
}
