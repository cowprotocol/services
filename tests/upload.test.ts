import { RpcBundle } from "../src/models";
import { convertBundle } from "../src/upload";

describe("testing bundle conversion", () => {
  test("succesfully decodes raw transaction", () => {
    const bundle: RpcBundle = {
      txs: [
        "0x02f8b10181db8459682f00850c9f5014d282be9894a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4880b844a9059cbb0000000000000000000000005408b27504dfcf7b0c3edf116e847aa19ce7f03c0000000000000000000000000000000000000000000000000000001e449a9400c080a049c0f50df4219481e031ac35816946daef9d08004f3324f7f46f6938488025aba02a4bda81f792bc5b7033804e39b7e55e619e56de1afcddd2ae4943ae5e7737c4",
      ],
      blockNumber: "0x1",
    };
    const result = convertBundle(bundle, "42");
    expect(result).toStrictEqual({
      bundleId: "42",
      blockNumber: 1,
      transactions: [
        {
          nonce: 219,
          maxPriorityFeePerGas: "1500000000",
          maxFeePerGas: "54212433106",
          gasPrice: undefined,
          gasLimit: "48792",
          to: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
          from: "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",
          value: "0",
          data: "0xa9059cbb0000000000000000000000005408b27504dfcf7b0c3edf116e847aa19ce7f03c0000000000000000000000000000000000000000000000000000001e449a9400",
          hash: "0x07151ed9706e4dffb31eaaac2ed1be5c6f05a9eef63c8f7c6ecad9ca8731aa22",
        },
      ],
    });
  });
});
