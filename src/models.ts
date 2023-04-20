export interface JsonRpcRequest {
  jsonrpc: string;
  id: string;
  method: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  params: Array<any>;
}

export interface RpcBundle {
  txs: Array<string>;
  blockNumber: string;
}

export interface DuneBundleTransaction {
  nonce: number;
  maxFeePerGas: string | undefined;
  maxPriorityFeePerGas: string | undefined;
  gasPrice: string | undefined;
  gasLimit: string;
  to: string;
  from: string;
  value: string;
  data: string;
  hash: string;
}

export interface DuneBundle {
  bundleId: string;
  blockNumber: number;
  transactions: Array<DuneBundleTransaction>;
}
