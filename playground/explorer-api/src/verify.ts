import { promises as fs } from 'fs';
import path from 'path';

// sourcify integration with v2 API support
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

// Local repository functions remain the same
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

// Remote (HTTP) sourcify v2 API helpers
export type RemoteRepoMatch = {
  type: 'full' | 'partial';
  chainId: number;
  address: string;
  apiBase: string;
};

export type ContractV2Response = {
  chainId: string;
  address: string;
  creatorTxHash?: string;
  // Match status
  create2Args?: any;
  // Files
  files?: {
    found: string[];
    missing: string[];
  };
  // Compilation info
  compilationTarget?: string;
  compiler?: {
    version: string;
  };
  language?: string;
  // Library info
  libraries?: Record<string, string>;
  // Settings
  settings?: any;
  // Sources
  sources?: Record<string, any>;
  // ABI
  abi?: any[];
  // Storage layout
  storageLayout?: any;
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

// Query sourcify v2 server for verification status
export async function findRemoteRepoMatch(apiUrl: string, chainId: number, address: string): Promise<RemoteRepoMatch | null> {
  const addr = address.toLowerCase();
  const base = apiUrl.replace(/\/$/, '');
  
  // Use v2 API endpoint with correct interpolation
  const url = `${base}/v2/contract/${chainId}/${addr}`;
  
  try {
    const res = await fetch(url);
    if (res.ok) {
      const data = await res.json() as ContractV2Response;
      
      // v2 API returns contract details directly
      // Determine match type based on the response
      // The v2 API doesn't explicitly state full vs partial in the same way,
      // but we can infer from the response structure or default to full
      
      return {
        type: 'full', // v2 API typically returns fully verified contracts
        chainId,
        address: addr,
        apiBase: base
      };
    }
  } catch (err) {
    console.error(`Error fetching contract from Sourcify v2: ${err}`);
  }
  
  return null;
}

// Read contract details using v2 API
export async function readRemoteMetadata(match: RemoteRepoMatch): Promise<any | null> {
  const url = `${match.apiBase}/v2/contract/${match.chainId}/${match.address}`;
  const contract = await getJson<ContractV2Response>(url);
  
  if (!contract) return null;
  
  // Transform v2 response to match expected metadata format
  return {
    compiler: contract.compiler,
    language: contract.language,
    output: {
      abi: contract.abi,
      devdoc: {},
      userdoc: {}
    },
    settings: contract.settings,
    sources: contract.sources,
    version: 1
  };
}

// List contract sources using v2 API
export async function listRemoteSources(match: RemoteRepoMatch, metadata?: any): Promise<Array<{ path: string; content: string }>> {
  const out: Array<{ path: string; content: string }> = [];
  
  // Fetch the contract details from v2 API
  const url = `${match.apiBase}/v2/contract/${match.chainId}/${match.address}`;
  const contract = await getJson<ContractV2Response>(url);
  
  if (!contract || !contract.sources) {
    return out;
  }
  
  // v2 API includes sources directly in the response
  for (const [sourcePath, sourceData] of Object.entries(contract.sources)) {
    if (sourceData && typeof sourceData === 'object' && 'content' in sourceData) {
      out.push({ 
        path: sourcePath, 
        content: (sourceData as any).content || ''
      });
    }
  }
  
  return out;
}

// Alternative: Use the repository endpoint if you need the raw files
// Note: This endpoint structure might vary based on your Sourcify instance
export async function getRepositoryFiles(apiBase: string, chainId: number, address: string, matchType: 'full' | 'partial' = 'full'): Promise<Array<{ path: string; content: string }>> {
  const addr = address.toLowerCase();
  const matchPath = matchType === 'full' ? 'full_match' : 'partial_match';
  const baseUrl = `${apiBase}/repository/contracts/${matchPath}/${chainId}/${addr}`;
  
  const out: Array<{ path: string; content: string }> = [];
  
  // Try to fetch metadata.json first
  try {
    const metadataUrl = `${baseUrl}/metadata.json`;
    const res = await fetch(metadataUrl);
    if (res.ok) {
      const metadata = await res.json();
      
      // Extract source file paths from metadata
      if (metadata.sources) {
        for (const sourcePath of Object.keys(metadata.sources)) {
          const sourceUrl = `${baseUrl}/sources/${encodeURIComponent(sourcePath)}`;
          try {
            const sourceRes = await fetch(sourceUrl);
            if (sourceRes.ok) {
              const content = await sourceRes.text();
              out.push({ path: sourcePath, content });
            }
          } catch {}
        }
      }
    }
  } catch {}
  
  return out;
}