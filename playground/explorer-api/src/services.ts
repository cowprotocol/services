import { RpcClient } from './rpc.js';
import type { DB } from './db.js';
import { upsertBlock, upsertTx, getAddressSummary, getLatestBlocks, getLatestTxs, getAddressTxs, getBlockByNumber, getBlockByHash, getBlockTxs, getTxByHash } from './db.js';
import { functionSigs, eventSigs } from './signatures.js';
import { parseAbiItem, decodeFunctionData, decodeEventLog } from 'viem';
import { SourceMapRegistry } from './sourceMap.js';
import { findRepoMatch, readMetadata } from './verify.js';

export class BlocksService {
  constructor(private rpc: RpcClient, private db: DB) {}

  async getBlock(id: string) {
    if (id === 'latest') {
      return this.rpc.getBlockByNumber('latest', true);
    }
    if (id.startsWith('0x') && id.length === 66) {
      // Check DB first for block by hash
      const dbBlock = getBlockByHash(this.db, id) as any;
      if (dbBlock) {
        const txs = getBlockTxs(this.db, dbBlock.number);
        return {
          number: '0x' + dbBlock.number.toString(16),
          hash: dbBlock.hash,
          parentHash: dbBlock.parentHash,
          timestamp: '0x' + dbBlock.timestamp.toString(16),
          transactions: txs.map((t: any) => ({ hash: t.hash })),
        };
      }
      // Fallback to RPC
      const block = await this.rpc.getBlockByHash(id, true);
      if (block) upsertBlock(this.db, block);
      return block;
    }
    const num = Number(id);
    if (!Number.isNaN(num)) {
      // Check DB first for block by number
      const dbBlock = getBlockByNumber(this.db, num) as any;
      if (dbBlock) {
        const txs = getBlockTxs(this.db, dbBlock.number);
        return {
          number: '0x' + dbBlock.number.toString(16),
          hash: dbBlock.hash,
          parentHash: dbBlock.parentHash,
          timestamp: '0x' + dbBlock.timestamp.toString(16),
          transactions: txs.map((t: any) => ({ hash: t.hash })),
        };
      }
      // Fallback to RPC
      const block = await this.rpc.getBlockByNumber(BigInt(num), true);
      if (block) upsertBlock(this.db, block);
      return block;
    }
    return null;
  }

  async listLatest(limit = 20) {
    return getLatestBlocks(this.db, limit);
  }
}

export class TxService {
  constructor(private rpc: RpcClient, private db: DB) {}

  async getTransaction(hash: string) {
    // Check DB first for indexed transactions
    const dbTx = getTxByHash(this.db, hash) as any;
    if (dbTx) {
      // Construct minimal tx and receipt from DB
      const tx = {
        hash: dbTx.hash,
        blockNumber: dbTx.blockNumber ? '0x' + dbTx.blockNumber.toString(16) : null,
        transactionIndex: dbTx.txIndex ? '0x' + dbTx.txIndex.toString(16) : null,
        from: dbTx.fromAddr,
        to: dbTx.toAddr,
        value: dbTx.value,
        input: '0x', // Not stored in DB
      };
      const receipt = dbTx.status != null ? {
        status: '0x' + dbTx.status.toString(16),
        gasUsed: dbTx.gasUsed,
        blockNumber: tx.blockNumber,
        transactionIndex: tx.transactionIndex,
      } : null;
      return { tx, receipt };
    }
    // Fallback to RPC for new transactions
    const tx = await this.rpc.getTransaction(hash);
    if (!tx) return null;
    const receipt = await this.rpc.getReceipt(hash).catch(() => null);
    upsertTx(this.db, tx, receipt);
    return { tx, receipt };
  }

  async listLatest(limit = 20) {
    return getLatestTxs(this.db, limit);
  }

  async listByAddress(address: string, limit = 50) {
    return getAddressTxs(this.db, address.toLowerCase(), limit);
  }
}

export class SearchService {
  constructor(private blocks: BlocksService, private txs: TxService, private rpc: RpcClient) {}

  async search(q: string) {
    // tx hash
    if (q.startsWith('0x') && q.length === 66) {
      const res = await this.txs.getTransaction(q);
      if (res) return { type: 'tx', redirect: `/tx/${res.tx.hash}`, value: res };
    }
    // address
    if (q.startsWith('0x') && q.length === 42) {
      const code = await this.rpc.call<string>('eth_getCode', [q, 'latest']);
      return { type: 'address', redirect: `/address/${q}`, value: { address: q, isContract: code && code !== '0x' } };
    }
    // special keywords (latest)
    if (q.toLowerCase() === 'latest') {
      const block = await this.blocks.getBlock('latest');
      if (block) {
        const blockNum = parseInt(block.number, 16);
        return { type: 'block', redirect: `/block/${blockNum}`, value: block };
      }
    }
    // block number
    const n = Number(q);
    if (!Number.isNaN(n)) {
      const block = await this.blocks.getBlock(q);
      if (block) {
        const blockNum = parseInt(block.number, 16);
        return { type: 'block', redirect: `/block/${blockNum}`, value: block };
      }
    }
    return { type: 'unknown', value: null };
  }
}

export class TraceService {
  constructor(private rpc: RpcClient) {}

  async trace(hash: string, opts: { mode?: 'tree' | 'steps'; memory?: boolean; stack?: boolean }) {
    if (opts.mode === 'tree') {
      // Use callTracer for compact call tree
      return this.rpc.call<any>('debug_traceTransaction', [hash, { tracer: 'callTracer' }]);
    }
    // Default to structLogs (step trace). Can be large; allow toggles
    const config: any = {
      disableStorage: !opts.memory,
      disableMemory: !opts.memory,
      disableStack: !opts.stack,
    };
    return this.rpc.call<any>('debug_traceTransaction', [hash, config]);
  }

  async gasReport(hash: string, decodeFn: (addr: string, input: string) => Promise<string>) {
    const tree = await this.rpc.call<any>('debug_traceTransaction', [hash, { tracer: 'callTracer' }]);
    type Frame = { depth: number; to?: string; type?: string; gasUsed?: number; function?: string };
    const frames: Frame[] = [];
    const byContract: Record<string, number> = {};
    const byFunction: Record<string, number> = {};

    async function walk(node: any, depth: number) {
      const to = String(node.to || node.address || '').toLowerCase();
      const gasUsed = Number(node.gasUsed || 0);
      const input: string = node.input || '0x';
      const fn = to ? await decodeFn(to, input) : '(creation)';
      frames.push({ depth, to, type: node.type, gasUsed, function: fn });
      if (to) byContract[to] = (byContract[to] || 0) + gasUsed;
      const key = `${to}#${fn}`;
      byFunction[key] = (byFunction[key] || 0) + gasUsed;
      for (const c of node.calls || []) await walk(c, depth + 1);
    }
    await walk(tree, 0);
    return { frames, byContract, byFunction };
  }
}

export class AddressService {
  constructor(private rpc: RpcClient, private db: DB) {}
  async getSummary(address: string) {
    const addr = address.toLowerCase();
    const code = await this.rpc.call<string>('eth_getCode', [addr, 'latest']);
    const summary = getAddressSummary(this.db, addr);
    return { ...summary, isContract: code && code !== '0x' };
  }
}

export class DecodeService {
  constructor(private db: DB) {}

  decodeCalldata(data: string) {
    if (!data || data === '0x') return null;
    const sig = data.slice(0, 10).toLowerCase();
    const human = functionSigs[sig];
    if (!human) {
      return { methodId: sig, signature: null, args: null };
    }
    try {
      const abiItem = parseAbiItem(`function ${human}`);
      const decoded = decodeFunctionData({ abi: [abiItem], data: data as `0x${string}` });
      return { methodId: sig, signature: human, functionName: decoded.functionName, args: decoded.args };
    } catch (e) {
      return { methodId: sig, signature: human, args: null, error: 'decode_failed' };
    }
  }

  decodeLog(log: { topics: string[]; data: string }) {
    const topic0 = (log.topics?.[0] || '').toLowerCase();
    const human = eventSigs[topic0];
    if (!human) return { topic0, signature: null, args: null };
    try {
      const abiItem = parseAbiItem(`event ${human}`);
      const decoded = decodeEventLog({ abi: [abiItem], data: log.data as `0x${string}` , topics: log.topics as any });
      return { topic0, signature: human, eventName: (abiItem as any).name, args: decoded.args };
    } catch (e) {
      return { topic0, signature: human, args: null, error: 'decode_failed' };
    }
  }

  decodeLogWithAbi(abi: any[], log: { topics: string[]; data: string; address?: string }) {
    try {
      const decoded: any = decodeEventLog({ abi, data: log.data as `0x${string}`, topics: log.topics as any });
      return { address: log.address || '', eventName: decoded.eventName as string, args: decoded.args };
    } catch {
      return this.decodeLog(log);
    }
  }

  async decodeFunctionName(address: string, input: string, fetchAbi: (addr: string) => Promise<any[] | null>) {
    if (!input || input === '0x') return '(fallback)';
    const methodId = input.slice(0, 10).toLowerCase();
    const abi = await fetchAbi(address);
    if (abi) {
      try {
        const sig = abi.find((x: any) => x.type === 'function' && x.name);
        const decoded = decodeFunctionData({ abi, data: input as `0x${string}` });
        return decoded.functionName || methodId;
      } catch {}
    }
    return functionSigs[methodId] || methodId;
  }
}
