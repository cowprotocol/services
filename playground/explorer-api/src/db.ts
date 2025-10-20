import Database from 'better-sqlite3';

export type DB = ReturnType<typeof Database>;

export function openDb(path: string) {
  const db = new Database(path);
  db.pragma('journal_mode = WAL');
  db.exec(`
    CREATE TABLE IF NOT EXISTS blocks (
      number INTEGER PRIMARY KEY,
      hash TEXT UNIQUE,
      parentHash TEXT,
      timestamp INTEGER,
      txCount INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);
    CREATE TABLE IF NOT EXISTS txs (
      hash TEXT PRIMARY KEY,
      blockNumber INTEGER,
      txIndex INTEGER,
      fromAddr TEXT,
      toAddr TEXT,
      value TEXT,
      status INTEGER,
      gasUsed TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_txs_block ON txs(blockNumber, txIndex);
    CREATE INDEX IF NOT EXISTS idx_txs_from ON txs(fromAddr);
    CREATE INDEX IF NOT EXISTS idx_txs_to ON txs(toAddr);
    CREATE TABLE IF NOT EXISTS abis (
      address TEXT PRIMARY KEY,
      abi TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS meta (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );
  `);
  return db;
}

export function upsertBlock(db: DB, b: any) {
  const stmt = db.prepare(`
    INSERT INTO blocks(number, hash, parentHash, timestamp, txCount)
    VALUES(@number, @hash, @parentHash, @timestamp, @txCount)
    ON CONFLICT(number) DO UPDATE SET
      hash=excluded.hash,
      parentHash=excluded.parentHash,
      timestamp=excluded.timestamp,
      txCount=excluded.txCount
  `);
  stmt.run({
    number: Number(b.number),
    hash: b.hash,
    parentHash: b.parentHash,
    timestamp: Number(b.timestamp),
    txCount: Array.isArray(b.transactions) ? b.transactions.length : (b.transactions ?? []).length,
  });
}

export function upsertTx(db: DB, tx: any, receipt?: any) {
  const stmt = db.prepare(`
    INSERT INTO txs(hash, blockNumber, txIndex, fromAddr, toAddr, value, status, gasUsed)
    VALUES(@hash, @blockNumber, @txIndex, @fromAddr, @toAddr, @value, @status, @gasUsed)
    ON CONFLICT(hash) DO UPDATE SET
      blockNumber=excluded.blockNumber,
      txIndex=excluded.txIndex,
      fromAddr=excluded.fromAddr,
      toAddr=excluded.toAddr,
      value=excluded.value,
      status=excluded.status,
      gasUsed=excluded.gasUsed
  `);
  stmt.run({
    hash: tx.hash,
    blockNumber: tx.blockNumber ? Number(tx.blockNumber) : null,
    txIndex: tx.transactionIndex ? Number(tx.transactionIndex) : null,
    fromAddr: tx.from ? String(tx.from).toLowerCase() : null,
    toAddr: tx.to ? String(tx.to).toLowerCase() : null,
    value: tx.value,
    status: receipt?.status != null ? Number(receipt.status) : null,
    gasUsed: receipt?.gasUsed ?? null,
  });
}

export function getAddressSummary(db: DB, address: string) {
  const sent = db.prepare('SELECT COUNT(1) as c FROM txs WHERE fromAddr = ?').get(address) as any;
  const received = db.prepare('SELECT COUNT(1) as c FROM txs WHERE toAddr = ?').get(address) as any;
  const last = db
    .prepare('SELECT blockNumber as b FROM txs WHERE fromAddr = ? OR toAddr = ? ORDER BY b DESC LIMIT 1')
    .get(address, address) as any;
  return {
    address,
    txCount: Number((sent?.c || 0) + (received?.c || 0)),
    lastSeenBlock: last?.b ?? null,
  };
}

export function getBlockByNumber(db: DB, number: number) {
  return db.prepare('SELECT number, hash, parentHash, timestamp, txCount FROM blocks WHERE number = ?').get(number);
}

export function getBlockByHash(db: DB, hash: string) {
  return db.prepare('SELECT number, hash, parentHash, timestamp, txCount FROM blocks WHERE hash = ?').get(hash);
}

export function getBlockTxs(db: DB, blockNumber: number) {
  return db.prepare('SELECT hash FROM txs WHERE blockNumber = ? ORDER BY txIndex ASC').all(blockNumber);
}

export function getTxByHash(db: DB, hash: string) {
  return db.prepare('SELECT hash, blockNumber, txIndex, fromAddr, toAddr, value, status, gasUsed FROM txs WHERE hash = ?').get(hash.toLowerCase());
}

export function getLatestBlocks(db: DB, limit: number) {
  return db.prepare('SELECT number, hash, parentHash, timestamp, txCount FROM blocks ORDER BY number DESC LIMIT ?').all(limit);
}

export function getLatestTxs(db: DB, limit: number) {
  return db.prepare('SELECT hash, blockNumber, txIndex, fromAddr, toAddr, value, status, gasUsed FROM txs WHERE blockNumber IS NOT NULL ORDER BY blockNumber DESC, txIndex DESC LIMIT ?').all(limit);
}

export function getAddressTxs(db: DB, address: string, limit: number) {
  return db.prepare('SELECT hash, blockNumber, txIndex, fromAddr, toAddr, value, status, gasUsed FROM txs WHERE fromAddr = ? OR toAddr = ? ORDER BY blockNumber DESC, txIndex DESC LIMIT ?').all(address, address, limit);
}

export function getMeta(db: DB, key: string) {
  const row = db.prepare('SELECT value FROM meta WHERE key = ?').get(key) as any;
  return row?.value;
}

export function setMeta(db: DB, key: string, value: string) {
  db.prepare('INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value').run(key, value);
}

export function getAbi(db: DB, address: string): string | null {
  const row = db.prepare('SELECT abi FROM abis WHERE address = ?').get(address.toLowerCase()) as any;
  return row?.abi || null;
}

export function setAbi(db: DB, address: string, abiJson: string) {
  db.prepare('INSERT INTO abis(address, abi) VALUES(?, ?) ON CONFLICT(address) DO UPDATE SET abi=excluded.abi')
    .run(address.toLowerCase(), abiJson);
}

export function pruneHistory(db: DB, keepFromBlock: number) {
  // Delete older txs and blocks
  db.prepare('DELETE FROM txs WHERE blockNumber IS NOT NULL AND blockNumber < ?').run(keepFromBlock);
  db.prepare('DELETE FROM blocks WHERE number < ?').run(keepFromBlock);
}

// Background indexer note: the periodic indexer is implemented in server.ts
// using setInterval to poll new blocks and persist them via this module.
// (This sentinel comment is to make the integration explicit.)

// History pruning is governed by the HISTORY_LIMIT configuration (server.ts)
// which determines how many recent blocks/txs to retain.
