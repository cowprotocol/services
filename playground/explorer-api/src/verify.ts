import { promises as fs } from 'fs';
import path from 'path';

// sourcify integration
// This module reads verified contract metadata and sources from a local
// sourcify repository mount (see SOURCIFY_REPO_PATH in config) and can
// optionally fall back to the sourcify HTTP API when the local repository
// is not available. It supports both full and partial matches following
// the sourcify repository layout.

export type VerifyRepoPaths = {
  repoPath: string;
  chainId: number;
};

export type RepoMatch = {
  type: 'full' | 'partial';
  dir: string;
};

export async function findRepoMatch({ repoPath, chainId }: VerifyRepoPaths, address: string): Promise<RepoMatch | null> {
  const addr = address.toLowerCase();
  const full = path.join(repoPath, 'contracts', 'full_match', String(chainId), addr);
  const partial = path.join(repoPath, 'contracts', 'partial_match', String(chainId), addr);
  try {
    const st = await fs.stat(full);
    if (st.isDirectory()) return { type: 'full', dir: full };
  } catch {}
  try {
    const st = await fs.stat(partial);
    if (st.isDirectory()) return { type: 'partial', dir: partial };
  } catch {}
  return null;
}

export async function readMetadata(dir: string) {
  const m = path.join(dir, 'metadata.json');
  const data = await fs.readFile(m, 'utf8');
  return JSON.parse(data);
}

export async function listSources(dir: string): Promise<Array<{ path: string; content: string }>> {
  const srcDir = path.join(dir, 'sources');
  const out: Array<{ path: string; content: string }> = [];
  async function walk(rel: string) {
    const abs = path.join(srcDir, rel);
    const entries = await fs.readdir(abs, { withFileTypes: true }).catch(() => [] as any);
    for (const e of entries) {
      const pRel = path.join(rel, e.name);
      const pAbs = path.join(srcDir, pRel);
      if (e.isDirectory()) {
        await walk(pRel);
      } else if (e.isFile()) {
        const content = await fs.readFile(pAbs, 'utf8').catch(() => '');
        out.push({ path: pRel, content });
      }
    }
  }
  try { await walk(''); } catch {}
  return out;
}

// Remote (HTTP) sourcify helpers
export type RemoteRepoMatch = {
  type: 'full' | 'partial';
  apiBase: string; // e.g. http://sourcify:5555/files/contracts/full_match/1/0xabc...
};

async function getJson<T>(url: string): Promise<T | null> {
  try {
    const res = await fetch(url, { method: 'GET' });
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

// Query sourcify server for verification status and build a base path for files
export async function findRemoteRepoMatch(apiUrl: string, chainId: number, address: string): Promise<RemoteRepoMatch | null> {
  const addr = address.toLowerCase();
  const base = apiUrl.replace(/\/$/, '');
  // 1) Fast check endpoint first
  const url = `${base}/check-by-addresses?addresses=${encodeURIComponent(addr)}&chainIds=${encodeURIComponent(String(chainId))}`;
  type CheckResp = Array<{ address?: string; chainId?: string | number; status?: string }>;
  try {
    const data = await getJson<CheckResp>(url);
    if (Array.isArray(data) && data.length) {
      const item = data.find((x) => String(x.address || '').toLowerCase() === addr);
      const status = String(item?.status || '').toLowerCase();
      let kind: 'full' | 'partial' | null = null;
      if (status === 'perfect' || status === 'full' || status === 'full_match') kind = 'full';
      else if (status === 'partial' || status === 'partial_match') kind = 'partial';
      if (kind) {
        const prefix = kind === 'full' ? 'full_match' : 'partial_match';
        // Use repository paths for direct files access
        const apiBase = `${base}/repository/contracts/${prefix}/${chainId}/${addr}`;
        return { type: kind, apiBase };
      }
    }
  } catch {}
  // 2) Fallback to tree endpoint which reports matches even when check says false (e.g., proxies)
  type TreeResp = { status?: string; files?: string[] };
  const treeUrl = `${base}/files/tree/any/${chainId}/${addr}`;
  const tree = await getJson<TreeResp>(treeUrl);
  if (tree && (tree.status === 'perfect' || tree.status === 'full' || tree.status === 'partial')) {
    const kind = tree.status === 'partial' ? 'partial' : 'full';
    const prefix = kind === 'full' ? 'full_match' : 'partial_match';
    const apiBase = `${base}/repository/contracts/${prefix}/${chainId}/${addr}`;
    return { type: kind, apiBase };
  }
  return null;
}

export async function readRemoteMetadata(match: RemoteRepoMatch): Promise<any | null> {
  const m = `${match.apiBase}/metadata.json`;
  return await getJson<any>(m);
}

export async function listRemoteSources(match: RemoteRepoMatch, metadata?: any): Promise<Array<{ path: string; content: string }>> {
  const out: Array<{ path: string; content: string }> = [];
  // Prefer listing by metadata.sources keys, if present
  const sources = (metadata && metadata.sources && typeof metadata.sources === 'object') ? Object.keys(metadata.sources) : [];
  const base = match.apiBase + '/sources';
  const fetchFile = async (p: string) => {
    const url = `${base}/${encodeURI(p)}`;
    try {
      const res = await fetch(url);
      if (!res.ok) return null;
      return await res.text();
    } catch {
      return null;
    }
  };
  if (sources.length) {
    for (const p of sources) {
      const content = await fetchFile(p);
      if (content != null) out.push({ path: p, content });
    }
    return out;
  }
  // If metadata absent or lacks keys, try a few common roots (best-effort)
  const tryFiles = ['contract.sol', 'contract.json'];
  for (const f of tryFiles) {
    const content = await fetchFile(f);
    if (content != null) out.push({ path: f, content });
  }
  return out;
}
