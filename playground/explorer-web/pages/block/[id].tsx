import { useRouter } from 'next/router';
import useSWR from 'swr';

const fetcher = (url: string) => fetch(url).then((r) => r.json());

export default function BlockPage() {
  const router = useRouter();
  const { id } = router.query as { id?: string };
  const base = process.env.NEXT_PUBLIC_API_BASE || 'http://localhost:8081';
  const url = id ? `${base}/api/blocks/${id}` : null;
  const { data, error } = useSWR(url, fetcher);
  if (!id) return null;
  if (error) return <div>Error loading block</div>;
  if (!data) return <div>Loading...</div>;
  const number = parseInt(data.number, 16);
  return (
    <main style={{ maxWidth: 900, margin: '40px auto', fontFamily: 'Inter, system-ui, Arial' }}>
      <h2>Block #{number}</h2>
      <p>Hash: {data.hash}</p>
      <p>Parent: {data.parentHash}</p>
      <p>Timestamp: {parseInt(data.timestamp, 16)}</p>
      <h3>Transactions</h3>
      <ul>
        {data.transactions?.map((tx: any) => (
          <li key={tx.hash}>
            <a href={`/tx/${tx.hash}`}>{tx.hash}</a>
          </li>
        ))}
      </ul>
    </main>
  );
}

