import { promises as fs } from 'fs';
import path from 'path';
import { findRepoMatch, readMetadata, listSources } from './verify.js';

type SrcEntry = { start: number; length: number; fileIndex: number; jump: string };
type PcToSrc = Map<number, SrcEntry & { path?: string; line?: number; endLine?: number }>;

function hexToBytes(hex: string): Uint8Array {
  const s = hex.startsWith('0x') ? hex.slice(2) : hex;
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function disassemble(bytecodeHex: string): Array<{ pc: number; op: number; pushLen: number }> {
  const code = hexToBytes(bytecodeHex);
  const out: Array<{ pc: number; op: number; pushLen: number }> = [];
  let pc = 0;
  while (pc < code.length) {
    const op = code[pc];
    let pushLen = 0;
    if (op >= 0x60 && op <= 0x7f) pushLen = (op - 0x5f); // PUSH1..PUSH32
    out.push({ pc, op, pushLen });
    pc += 1 + pushLen;
  }
  return out;
}

function parseSourceMap(srcMapStr: string): SrcEntry[] {
  // format: start:length:file:jump; ...
  const out: SrcEntry[] = [];
  let last = { start: 0, length: 0, fileIndex: -1, jump: '' };
  for (const part of srcMapStr.split(';')) {
    if (part.length === 0) {
      out.push({ ...last });
      continue;
    }
    const seg = part.split(':');
    const start = seg[0] !== '' ? parseInt(seg[0], 10) : last.start;
    const length = seg[1] !== '' ? parseInt(seg[1], 10) : last.length;
    const fileIndex = seg[2] !== '' ? parseInt(seg[2], 10) : last.fileIndex;
    const jump = seg[3] !== '' ? seg[3] : last.jump;
    last = { start, length, fileIndex, jump };
    out.push(last);
  }
  return out;
}

function computeLineMap(content: string): number[] {
  // returns array of offsets where each line starts
  const lines = [0];
  for (let i = 0; i < content.length; i++) if (content[i] === '\n') lines.push(i + 1);
  return lines;
}

function offsetToLine(lines: number[], offset: number): number {
  // 1-based line number
  let lo = 0, hi = lines.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid] <= offset) lo = mid + 1; else hi = mid - 1;
  }
  return hi + 1; // 1-based
}

export class SourceMapRegistry {
  private cache: Map<string, { pcToSrc: PcToSrc } > = new Map();
  constructor(private repoPath: string, private chainId: number) {}

  async loadForAddress(address: string): Promise<{ pcToSrc: PcToSrc } | null> {
    const addr = address.toLowerCase();
    if (this.cache.has(addr)) return this.cache.get(addr)!;
    const match = await findRepoMatch({ repoPath: this.repoPath, chainId: this.chainId }, addr);
    if (!match) return null;
    const meta = await readMetadata(match.dir);
    const output = meta.output || {};
    const evm = output?.contracts ? undefined : undefined; // compatibility guard
    // try common locations
    const deployed = meta?.output?.contracts
      ? (() => {
          // find the compiled contract blob matching compilationTarget
          const ct = meta.settings?.compilationTarget;
          const file = ct && Object.keys(ct)[0];
          const name = file ? ct[file] : undefined;
          if (file && name && meta.output?.contracts?.[file]?.[name]?.evm?.deployedBytecode) {
            return meta.output.contracts[file][name].evm.deployedBytecode;
          }
          // fallback: best-effort scan for first entry having deployedBytecode
          for (const fileKey of Object.keys(meta.output?.contracts || {})) {
            const contracts = meta.output.contracts[fileKey];
            for (const cname of Object.keys(contracts || {})) {
              const d = contracts[cname]?.evm?.deployedBytecode;
              if (d?.object && d?.sourceMap) return d;
            }
          }
          return null;
        })()
      : meta?.deployedBytecode || null;

    const object = deployed?.object || deployed?.bytecode || deployed?.object?.object;
    const srcMapStr = deployed?.sourceMap || deployed?.srcMap;
    if (!object || !srcMapStr) return null;

    const dis = disassemble(object);
    const srcMap = parseSourceMap(srcMapStr);
    const pcToSrc: PcToSrc = new Map();

    // Build id->path and path->content maps
    const sourcesMeta = (meta.output?.sources) || (meta.sources) || {};
    const idToPath: Record<number, string> = {};
    const pathToContent: Record<string, string> = {};
    for (const key of Object.keys(sourcesMeta)) {
      const id = sourcesMeta[key]?.id;
      if (typeof id === 'number') idToPath[id] = key;
    }
    const files = await listSources(match.dir);
    for (const f of files) pathToContent[f.path] = f.content;
    const lineCache: Record<string, number[]> = {};

    for (let i = 0; i < dis.length && i < srcMap.length; i++) {
      const pc = dis[i].pc;
      const s = srcMap[i];
      const p = idToPath[s.fileIndex];
      const entry: any = { ...s };
      if (p) {
        entry.path = p;
        const content = pathToContent[p];
        if (content) {
          const lines = lineCache[p] || (lineCache[p] = computeLineMap(content));
          entry.line = offsetToLine(lines, s.start);
          entry.endLine = offsetToLine(lines, s.start + Math.max(0, s.length - 1));
        }
      }
      pcToSrc.set(pc, entry);
    }

    const record = { pcToSrc };
    this.cache.set(addr, record);
    return record;
  }
}

