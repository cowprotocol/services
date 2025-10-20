import { useRouter } from 'next/router';
import useSWR from 'swr';
import { useState } from 'react';

const fetcher = (url: string) => fetch(url).then((r) => r.json());

export default function TxPage() {
  const router = useRouter();
  const { hash } = router.query as { hash?: string };
  const base = process.env.NEXT_PUBLIC_API_BASE || 'http://localhost:8081';
  const url = hash ? `${base}/api/tx/${hash}` : null;
  const { data, error } = useSWR(url, fetcher);
  const traceUrl = hash ? `${base}/api/tx/${hash}/trace?mode=tree` : null;
  const { data: trace } = useSWR(traceUrl, fetcher);
  const [from, setFrom] = useState(0);
  const [size, setSize] = useState(200);
  const stepsUrl = hash ? `${base}/api/tx/${hash}/steps?from=${from}&to=${from+size}` : null;
  const gasUrl = hash ? `${base}/api/tx/${hash}/gas-report` : null;
  const { data: steps } = useSWR(stepsUrl, fetcher);
  const { data: gas } = useSWR(gasUrl, fetcher);
  if (!hash) return null;
  if (error) return <div>Error loading transaction</div>;
  if (!data) return <div>Loading...</div>;
  const { tx, receipt } = data;
  return (
    <main style={{ maxWidth: 900, margin: '40px auto', fontFamily: 'Inter, system-ui, Arial' }}>
      <h2>Transaction</h2>
      <p>Hash: {tx.hash}</p>
      <p>Block: {tx.blockNumber ? parseInt(tx.blockNumber, 16) : 'pending'}</p>
      <p>From: {tx.from}</p>
      <p>To: {tx.to}</p>
      <p>Value: {tx.value}</p>
      <h3>Receipt</h3>
      {receipt ? (
        <>
          <p>Status: {receipt.status}</p>
          <p>Gas Used: {receipt.gasUsed}</p>
          <p>Logs: {receipt.logs?.length || 0}</p>
        </>
      ) : (
        <p>Pending</p>
      )}
      <h3>Decoded Input</h3>
      <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>
        {data.decodedInput ? JSON.stringify(data.decodedInput, null, 2) : 'No input'}
      </pre>
      <h3>Decoded Logs</h3>
      <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>
        {data.decodedLogs ? JSON.stringify(data.decodedLogs, null, 2) : 'No logs'}
      </pre>
      <h2>Debug</h2>
      <h3>Call Tree</h3>
      <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>{trace ? JSON.stringify(trace, null, 2) : 'Loading...'}</pre>

      <h3>Gas Report</h3>
      <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto' }}>{gas ? JSON.stringify(gas, null, 2) : 'Loading...'}</pre>

      <h3>Stepper (structLogs slice)</h3>
      <div style={{ display: 'flex', gap: 8 }}>
        <label>From: <input type="number" value={from} onChange={e => setFrom(parseInt(e.target.value||'0',10))} style={{ width: 120 }} /></label>
        <label>Size: <input type="number" value={size} onChange={e => setSize(parseInt(e.target.value||'200',10))} style={{ width: 120 }} /></label>
      </div>
      <pre style={{ background: '#f6f8fa', padding: 12, overflow: 'auto', maxHeight: 500 }}>
        {steps ? JSON.stringify(steps, null, 2) : 'Loading...'}
      </pre>
    </main>
  );
}
