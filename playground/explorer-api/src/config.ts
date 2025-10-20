export type AppConfig = {
  rpcUrl: string;
  dbPath: string;
  port: number;
  networkName: string;
  enableTraceSteps: boolean;
  sourcifyApiUrl: string;
  sourcifyRepoPath: string;
  chainId: number;
  indexerIntervalMs: number;
  indexerBatch: number;
  historyLimit: number;
};

export function loadConfig(): AppConfig {
  const rpcUrl = process.env.JSON_RPC_URL || 'http://localhost:8545';
  const dbPath = process.env.DB_PATH || '/data/explorer.sqlite';
  const port = parseInt(process.env.PORT || '8081', 10);
  const networkName = process.env.NETWORK_NAME || 'local';
  const enableTraceSteps = (process.env.ENABLE_TRACE_STEPS || 'false') === 'true';
  const sourcifyApiUrl = process.env.SOURCIFY_API_URL || 'http://sourcify:5555';
  const sourcifyRepoPath = process.env.SOURCIFY_REPO_PATH || '/sourcify/repository';
  const chainId = parseInt(process.env.CHAIN_ID || '1', 10);
  const indexerIntervalMs = parseInt(process.env.INDEXER_INTERVAL_MS || '1500', 10);
  const indexerBatch = parseInt(process.env.INDEXER_BATCH || '50', 10);
  const historyLimit = parseInt(process.env.HISTORY_LIMIT || '10000', 10);
  return { rpcUrl, dbPath, port, networkName, enableTraceSteps, sourcifyApiUrl, sourcifyRepoPath, chainId, indexerIntervalMs, indexerBatch, historyLimit };
}
