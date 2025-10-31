import Fastify from 'fastify';
import cors from '@fastify/cors';
import { Registry, collectDefaultMetrics } from 'prom-client';
import { loadConfig } from './config.js';
import { RpcClient } from './rpc.js';
import { openDb } from './db.js';
import { BlocksService, TxService, SearchService, TraceService, AddressService, DecodeService } from './services.js';
import { setMeta, getMeta, upsertBlock, upsertTx, getAbi, setAbi, pruneHistory } from './db.js';
import { findRepoMatch, readMetadata, listSources, findRemoteRepoMatch, readRemoteMetadata, listRemoteSources } from './verify.js';
import { SourceMapRegistry } from './sourceMap.js';

const config = loadConfig();
const app = Fastify({ logger: true });
await app.register(cors, { origin: true });

const registry = new Registry();
collectDefaultMetrics({ register: registry });

const rpc = new RpcClient(config.rpcUrl);
const db = openDb(config.dbPath);
const blocksService = new BlocksService(rpc, db);
const txService = new TxService(rpc, db);
const searchService = new SearchService(blocksService, txService, rpc);
const traceService = new TraceService(rpc);
const addressService = new AddressService(rpc, db);
const decodeService = new DecodeService(db);
const smRegistry = new SourceMapRegistry(config.sourcifyRepoPath, config.chainId);

app.get('/healthz', async () => ({ ok: true, network: config.networkName }));

app.get('/metrics', async (_req, reply) => {
  reply.header('Content-Type', registry.contentType);
  return registry.metrics();
});

app.get('/api/blocks/:id', async (req, reply) => {
  const id = String((req.params as any).id);
  const block = await blocksService.getBlock(id);
  if (!block) return reply.code(404).send({ error: 'Not found' });
  return block;
});

app.get('/api/blocks', async (req) => {
  const limit = Math.min(parseInt(String((req.query as any).limit || '20'), 10), 100);
  return await blocksService.listLatest(limit);
});

app.get('/api/tx/:hash', async (req, reply) => {
  const { hash } = req.params as any;
  const data = await txService.getTransaction(hash);
  if (!data) return reply.code(404).send({ error: 'Not found' });
  const decodedInput = decodeService.decodeCalldata(data.tx.input);
  // Decode logs using verified ABIs where available, else fallback to signature map
  let decodedLogs: any[] = [];
  if (data.receipt?.logs?.length) {
    async function loadAbi(addr: string): Promise<string | null> {
      let abi = getAbi(db, addr);
      if (abi) return abi;
      // Try local sourcify repo
      const match = await findRepoMatch({ repoPath: config.sourcifyRepoPath, chainId: config.chainId }, addr);
      if (match) {
        const meta = await readMetadata(match.dir);
        abi = JSON.stringify(meta.output?.abi || meta.abi || []);
        if (abi) setAbi(db, addr, abi);
        return abi || null;
      }
      // Try remote sourcify API
      const r = await findRemoteRepoMatch(config.sourcifyApiUrl, config.chainId, addr).catch(() => null as any);
      if (r) {
        const meta = await readRemoteMetadata(r);
        const arr = meta?.output?.abi || meta?.abi || [];
        if (arr) {
          abi = JSON.stringify(arr);
          setAbi(db, addr, abi);
          return abi;
        }
      }
      return null;
    }
    decodedLogs = await Promise.all(
      data.receipt.logs.map(async (log: any) => {
        try {
          const addr = String(log.address || '').toLowerCase();
          const abi = await loadAbi(addr);
          if (abi) {
            return decodeService.decodeLogWithAbi(JSON.parse(abi), log);
          } else {
            return decodeService.decodeLog(log);
          }
        } catch {
          return null;
        }
      })
    );
  }
  return { ...data, decodedInput, decodedLogs };
});

app.get('/api/tx', async (req) => {
  const limit = Math.min(parseInt(String((req.query as any).limit || '20'), 10), 100);
  return await txService.listLatest(limit);
});

app.get('/api/tx/:hash/trace', async (req, reply) => {
  const { hash } = req.params as any;
  const { mode = 'tree', memory = '0', stack = '0' } = (req.query as any) || {};
  try {
    const trace = await traceService.trace(hash, {
      mode: mode === 'steps' ? 'steps' : 'tree',
      memory: String(memory) === '1',
      stack: String(stack) === '1',
    });
    return trace;
  } catch (e: any) {
    return reply.code(400).send({ error: e?.message || 'trace error' });
  }
});

// Paginated steps with source mapping where possible
app.get('/api/tx/:hash/steps', async (req, reply) => {
  const { hash } = req.params as any;
  const { from = '0', to = '200', memory = '0', stack = '0' } = (req.query as any) || {};
  try {
    const res = await traceService.trace(hash, {
      mode: 'steps',
      memory: String(memory) === '1',
      stack: String(stack) === '1',
    });
    const logs: any[] = res.structLogs || res.result?.structLogs || [];
    const f = Math.max(0, parseInt(String(from), 10));
    const t = Math.min(logs.length, parseInt(String(to), 10));
    // Build depth->address mapping by scanning and decoding CALL-like ops
    const depthAddr: Record<number, string> = {};
    // Seed depth 1 with tx.to when possible
    try {
      const txData = await txService.getTransaction(hash);
      if (txData?.tx?.to) depthAddr[1] = String(txData.tx.to).toLowerCase();
    } catch {}

    function getCallee(op: string, stackArr: string[]): string | null {
      const s = stackArr || [];
      const nthFromTop = (n: number) => s.length > n ? s[s.length - 1 - n] : null;
      let addrHex: string | null = null;
      switch (op) {
        case 'CALL':
          addrHex = nthFromTop(5); // [-6]
          break;
        case 'CALLCODE':
          addrHex = nthFromTop(5);
          break;
        case 'STATICCALL':
        case 'DELEGATECALL':
          addrHex = nthFromTop(4); // [-5]
          break;
        default:
          return null;
      }
      if (!addrHex) return null;
      let v = addrHex.toLowerCase();
      if (v.startsWith('0x')) v = v.slice(2);
      v = v.padStart(40, '0').slice(-40);
      return '0x' + v;
    }

    const slice = [] as any[];
    for (let i = f; i < t; i++) {
      const log = logs[i];
      const depth = Number(log.depth || 0);
      // On CALL-like ops, compute callee for next depth
      if (['CALL', 'CALLCODE', 'STATICCALL', 'DELEGATECALL'].includes(log.op)) {
        const toAddr = getCallee(log.op, log.stack || []);
        if (toAddr) depthAddr[depth + 1] = toAddr;
      }
      const addr = depthAddr[depth];
      let src: any = null;
      if (addr) {
        const mapper = await smRegistry.loadForAddress(addr).catch(() => null as any);
        if (mapper?.pcToSrc?.has(log.pc)) src = mapper.pcToSrc.get(log.pc);
      }
      slice.push({ idx: i, pc: log.pc, op: log.op, gasCost: log.gasCost, gas: log.gas, depth, address: addr, src,
        stack: String(stack) === '1' ? log.stack : undefined,
        memory: String(memory) === '1' ? log.memory : undefined,
      });
    }
    return { total: logs.length, from: f, to: t, steps: slice };
  } catch (e: any) {
    return reply.code(400).send({ error: e?.message || 'steps error' });
  }
});

// Gas report aggregated by call frames
app.get('/api/tx/:hash/gas-report', async (req, reply) => {
  const { hash } = req.params as any;
  try {
    const report = await traceService.gasReport(hash, async (addr, input) => {
      let abiStr = getAbi(db, addr);
      if (!abiStr) {
        const match = await findRepoMatch({ repoPath: config.sourcifyRepoPath, chainId: config.chainId }, addr);
        if (match) {
          const meta = await readMetadata(match.dir);
          abiStr = JSON.stringify(meta.output?.abi || meta.abi || []);
          setAbi(db, addr, abiStr);
        } else {
          const r = await findRemoteRepoMatch(config.sourcifyApiUrl, config.chainId, addr).catch(() => null as any);
          if (r) {
            const meta = await readRemoteMetadata(r);
            const arr = meta?.output?.abi || meta?.abi || [];
            if (arr) {
              abiStr = JSON.stringify(arr);
              setAbi(db, addr, abiStr);
            }
          }
        }
      }
      const abi = abiStr ? JSON.parse(abiStr) : null;
      return await decodeService.decodeFunctionName(addr, input, async () => abi);
    });
    return report;
  } catch (e: any) {
    return reply.code(400).send({ error: e?.message || 'gas report error' });
  }
});

app.get('/api/search', async (req) => {
  const q = String((req.query as any).q || '').trim();
  return await searchService.search(q);
});

app.get('/api/address/:address', async (req) => {
  const { address } = req.params as any;
  return await addressService.getSummary(address);
});

app.get('/api/address/:address/txs', async (req) => {
  const { address } = req.params as any;
  const limit = Math.min(parseInt(String((req.query as any).limit || '50'), 10), 200);
  return await txService.listByAddress(address, limit);
});

app.get('/api/decode/calldata', async (req) => {
  const { data } = (req.query as any) || {};
  return decodeService.decodeCalldata(String(data || ''));
});

// Verification & ABI endpoints
app.get('/api/abi/:address', async (req, reply) => {
  const { address } = req.params as any;
  const addr = String(address).toLowerCase();
  let abi = getAbi(db, addr);
  if (!abi) {
    const match = await findRepoMatch({ repoPath: config.sourcifyRepoPath, chainId: config.chainId }, addr);
    if (match) {
      const meta = await readMetadata(match.dir);
      abi = JSON.stringify(meta.output?.abi || meta.abi || []);
      setAbi(db, addr, abi);
    } else {
      const r = await findRemoteRepoMatch(config.sourcifyApiUrl, config.chainId, addr).catch(() => null as any);
      if (!r) return reply.code(404).send({ error: 'not_verified' });
      const meta = await readRemoteMetadata(r);
      const arr = meta?.output?.abi || meta?.abi || [];
      abi = JSON.stringify(arr || []);
      setAbi(db, addr, abi);
    }
  }
  return JSON.parse(abi);
});

app.get('/api/source/:address', async (req, reply) => {
  const { address } = req.params as any;
  const addr = String(address).toLowerCase();
  const match = await findRepoMatch({ repoPath: config.sourcifyRepoPath, chainId: config.chainId }, addr);
  if (match) {
    const meta = await readMetadata(match.dir);
    const sources = await listSources(match.dir);
    return { verified: true, type: match.type, metadata: meta, sources };
  }
  // Remote fallback
  const r = await findRemoteRepoMatch(config.sourcifyApiUrl, config.chainId, addr).catch(() => null as any);
  if (!r) return reply.code(404).send({ verified: false });
  const meta = await readRemoteMetadata(r);
  const sources = await listRemoteSources(r, meta);
  return { verified: true, type: r.type, metadata: meta, sources };
});

app.get('/api/verify/status/:address', async (req) => {
  const { address } = req.params as any;
  const addr = String(address).toLowerCase();
  const match = await findRepoMatch({ repoPath: config.sourcifyRepoPath, chainId: config.chainId }, addr);
  if (match) return { verified: true, type: match.type };
  const r = await findRemoteRepoMatch(config.sourcifyApiUrl, config.chainId, addr).catch(() => null as any);
  return { verified: !!r, type: r?.type || null };
});

app.listen({ port: config.port, host: '0.0.0.0' }).then(() => {
  app.log.info(`Explorer API listening on :${config.port}`);
});

// Lightweight indexer: poll latest block and fill DB
async function runIndexer() {
  const logger = app.log;
  try {
    const latestHex = await rpc.getBlockNumber();
    const latest = Number(latestHex);
    const lastSynced = Number(getMeta(db, 'last_synced') || '0');
    let next = lastSynced === 0 ? latest : lastSynced + 1;
    const maxCatchup = config.indexerBatch; // prevent long catchup bursts
    const end = Math.min(latest, next + maxCatchup);
    for (; next <= end; next++) {
      const block = await rpc.getBlockByNumber(BigInt(next), true);
      if (!block) continue;
      upsertBlock(db, block);
      // Upsert txs and their receipts (bounded concurrency)
      const txs = block.transactions || [];
      const concurrency = 5;
      for (let i = 0; i < txs.length; i += concurrency) {
        const batch = txs.slice(i, i + concurrency);
        await Promise.all(
          batch.map(async (tx: any) => {
            try {
              const receipt = await rpc.getReceipt(tx.hash).catch(() => null);
              upsertTx(db, tx, receipt);
            } catch (e) {
              logger.warn({ err: e }, 'receipt upsert failed');
            }
          })
        );
      }
      setMeta(db, 'last_synced', String(next));
    }
    // Prune history beyond configured limit
    const keepFrom = Math.max(0, latest - config.historyLimit);
    pruneHistory(db, keepFrom);
  } catch (e) {
    app.log.debug({ err: e }, 'indexer step error');
  }
}

setInterval(runIndexer, config.indexerIntervalMs);
