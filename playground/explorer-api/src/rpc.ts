type JsonRpcRequest = {
  jsonrpc: '2.0';
  id: number | string;
  method: string;
  params: any[];
};

export class RpcClient {
  private url: string;
  private id = 1;

  constructor(url: string) {
    this.url = url;
  }

  async call<T>(method: string, params: any[] = []): Promise<T> {
    const req: JsonRpcRequest = {
      jsonrpc: '2.0',
      id: this.id++,
      method,
      params,
    };
    const res = await fetch(this.url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(req),
    });
    if (!res.ok) {
      throw new Error(`RPC HTTP error ${res.status}`);
    }
    const body = await res.json();
    if (body.error) {
      throw new Error(`RPC error: ${body.error.code} ${body.error.message}`);
    }
    return body.result as T;
  }

  // Helpers
  async getBlockByNumber(blockTag: string | bigint, includeTx = true): Promise<any> {
    const tag = typeof blockTag === 'bigint' ? `0x${blockTag.toString(16)}` : blockTag;
    return this.call<any>('eth_getBlockByNumber', [tag, includeTx]);
  }

  async getBlockByHash(hash: string, includeTx = true): Promise<any> {
    return this.call<any>('eth_getBlockByHash', [hash, includeTx]);
  }

  async getBlockNumber(): Promise<bigint> {
    const hex = await this.call<string>('eth_blockNumber');
    return BigInt(hex);
  }

  async getTransaction(hash: string): Promise<any> {
    return this.call<any>('eth_getTransactionByHash', [hash]);
  }

  async getReceipt(hash: string): Promise<any> {
    return this.call<any>('eth_getTransactionReceipt', [hash]);
  }

  async debugTraceTransaction(hash: string, cfg?: any): Promise<any> {
    return this.call<any>('debug_traceTransaction', [hash, cfg ?? {}]);
  }
}
