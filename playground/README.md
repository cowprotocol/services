# CoW Playground

The ultimate goal is to have a single command that will spin up a local CoW network, with all the components needed to
run the CoW Protocol stack 🚀

## Quickstart

1. Clone this repo.
2. It is expected that this is run from a `devcontainer` in VSCode, or a similar environment.
3. Configure the stack by editing the `.env.example` file and renaming it to `.env`. **NOTE**: RPC demand is very high,
   for optimal performance, use a local node. The stack was tested with `reth` on `mainnet`.
4. Run `docker-compose -f docker-compose.fork.yml up -d`.
5. Configure Rabby Wallet (or see [Metamask specific notes](#metamask)) to use the RPC endpoint at
   `http://localhost:8545` (for `mainnet`, or your network of choice).
6. Configure your wallet to use a test account (any of the first 10 accounts from the test mnemonic will do).

**NOTE**: By default, `anvil` will set the balances of the first 10 accounts to 10000 ETH. The wallet configuration is:

```
Mnemonic:           test test test test test test test test test test test junk
Derivation path:    m/44'/60'/0'/0/

Private Keys
==================

(0) 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
(1) 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
(2) 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
(3) 0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
(4) 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
(5) 0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
(6) 0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e
(7) 0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356
(8) 0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97
(9) 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6
```

Now with Rabby configured, and the services started, you can browse to http://localhost:8000 and make a trade with CoW Swap.

> Initially you will start with 10000 ETH, to run proper transaction you will need to wrap some ETH first.
> The EthFlow is not configured by default, the next section explains how to set it up.
> You can follow along with watching the logs of the `autopilot`, `driver`, and `baseline` solver to see how the Protocol interacts.

## Setting up EthFlow

> By default, the EthFlow is setup by default, but this section will cover how to configure it for troubleshooting purposes.

For EthFlow orders to work, you will need to provide an EthFlow contract address.

You do that by adding `ETHFLOW_CONTRACTS=<address>` to your `.env` file.

> For testing, (at the time of writing) the contract you should use is [`0x04501b9b1d52e67f6862d157e00d13419d2d6e95`](https://github.com/cowprotocol/cow-sdk/blob/747e2ade0118d2468c00af7d773bb7b1fbf64f66/src/common/consts/contracts.ts#L56),
> as time progresses it may no longer be the case, there are multiple ways of figuring out the right contract.


### Setting the contract — using the frontend repo

Inside the [`contracts.ts`](https://github.com/cowprotocol/cow-sdk/blob/main/src/common/consts/contracts.ts)
file you will find the address for the EthFlow staging contract:

```ts
const BARN_ETH_FLOW_ADDRESS = '0x04501b9b1d52e67f6862d157e00d13419d2d6e95'
```

This is, of course, subject to change, alternatively you can follow the next method.

#### Setting the contract — the long way

To go the long way, you first run the system and try to perform an EthFlow transaction.
The expect behavior is for the ETH to be sent but for the flow to block on the order creation.

Copy the transaction hash from "View Transaction", the link should point to Etherscan but the transaction should be missing
(you're running a fork after all), the link should be similar to the following:

```
https://etherscan.io/tx/0xe510b729782d5c19b6cceebec905a20b749519af295a32a06e3122e46221fd4d
```

You should take the hash and use `cast` to look for its receipt:

```
$ cast receipt 0xe510b729782d5c19b6cceebec905a20b749519af295a32a06e3122e46221fd4d

blockHash            0xc36ec230da90e097d4edb63167cbfe62cdf0e5d515a5bbd282372a7dd20e74e0
blockNumber          23197203
contractAddress
cumulativeGasUsed    56228
effectiveGasPrice    7782057214
from                 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
gasUsed              56228
logs                 [{"address":"0x04501b9b1d52e67f6862d157e00d13419d2d6e95","topics":["0xcf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9","0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"],"data":"0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb922660000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000000000000000000000000000000000010943cf0e00000000000000000000000000000000000000000000000000000000ffffffffd2b043c2cbd6f15b991059ab1e7bb61a27834f458a0986a5bcb9d893f69cbe990000000000000000000000000000000000000000000000000000000000000000f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677500000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001c0000000000000000000000000000000000000000000000000000000000000024000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000001404501b9b1d52e67f6862d157e00d13419d2d6e95000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c000000000000000368a884e60000000000000000000000000000000000000000","blockHash":"0xc36ec230da90e097d4edb63167cbfe62cdf0e5d515a5bbd282372a7dd20e74e0","blockNumber":"0x161f613","blockTimestamp":"0x68a87deb","transactionHash":"0xe510b729782d5c19b6cceebec905a20b749519af295a32a06e3122e46221fd4d","transactionIndex":"0x0","logIndex":"0x0","removed":false}]
logsBloom            0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000200000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000200000000000000000000000002000000000000100000000000000000000000000000000000000000800000000000000000000000000000000
root
status               1 (success)
transactionHash      0xe510b729782d5c19b6cceebec905a20b749519af295a32a06e3122e46221fd4d
transactionIndex     0
type                 2
blobGasPrice         1
blobGasUsed
to                   0x04501b9b1D52e67f6862d157E00D13419D2D6E95
```

The key field is the last one, `to`, that address is the smart contract address for the EthFlow.


### Set the indexing start — Optional

Indexing takes a while, to give the playground a little boost you should query the current chain head from "final" RPC you're using:

```
cast block-number -r <RPC_URL>
```

For example:
```
cast block-number -r wss://mainnet.gateway.tenderly.co
23197382
```

Finally, you set `ETHFLOW_INDEXING_START` to the received block number and you're good to go.

### Skipping indexing

You can set `SKIP_EVENT_SYNC=true` to skip the block sync. This is set by default in the playground.

### Resetting the playground

Resetting the playground involves resetting the state for both the containers, but _also_ resetting the state for your
wallet! Failure to do this may result in unexpected behaviour due to `nonce` issues:

1. Remove the containers and volumes with `docker-compose -f docker-compose.fork.yml down --remove-orphans --volumes`.
2. Reset your wallet:
   a. For Rabby, select "Clear pending" from the "More" section of the wallet.
   b. For Metamask,
   follow [these instructions](https://support.metamask.io/hc/en-us/articles/360015488891-How-to-clear-your-account-activity-reset-account)
   to reset your account.

### Try another network

Woah! You're quick. You've already got the CoW stack running on `mainnet`! But what if you want to try another network?
No problem!

1. Change the `ETH_RPC_URL` in the `.env` file to the network of your choice.
2. Reset the stack by removing the containers and volumes with
   `docker-compose -f docker-compose.fork.yml down --remove-orphans --volumes`.
3. Start the stack again with `docker-compose -f docker-compose.fork.yml up -d`.

## Web3 wallets

### Rabby

Rabby is a web3 wallet and has some nice features for interacting with the CoW Protocol.
It's suggested to use this wallet for the best experience.
When interacting with the CoW Swap UI, just select Metamask when connecting a wallet, and this will use the Rabby
wallet.

### Metamask

Metamask is popular, and unfortunately Rabby isn't available on Firefox.
Also, unfortunately Metamask take a very strong stance on not allowing you change the RPC endpoint for `mainnet` within
the user interface.
Let's use some skills to get around this!
Open up your browser's developer console, and run the following:

```javascript
await window.ethereum.request({
  method: 'wallet_addEthereumChain',
  params: [
    {
      chainId: '0x1',
      chainName: 'Local Network (Mainnet)',
      rpcUrls: ['http://localhost:8545'],
      nativeCurrency: {
        name: "Ethereum",
        symbol: "ETH",
        decimals: 18,
      },
    },
  ],
});
```

## Components

| **Component** | **Container name** | **Host port** | **Container port** | **Stack**  |
|---------------|--------------------|---------------|--------------------|------------|
| Autopilot     | autopilot          | N/A           | N/A                | Common     |
| Driver        | driver             | N/A           | 80                 | Common     |
| Baseline      | baseline           | N/A           | 80                 | Common     |
| CoW Swap      | cowswap            | 8000          | 80                 | Local/Fork |
| CoW Explorer  | cowexplorer        | 8001          | 80                 | Local/Fork |
| Orderbook     | orderbook          | 8080          | 80                 | Local/Fork |
| RPC           | chain              | 8545          | 8545               | Local/Fork |
| Postgres      | postgres           | 5432          | 5432               | Local/Fork |
| Adminer       | adminer            | 8082          | 8080               | Local/Fork |
| Grafana       | grafana            | 3000          | 3000               | Local/Fork |

**NOTE**: Currently only **FORK** mode is supported.

## Modes

### Shadow

**NOT YET IMPLEMENTED**

In this mode, the stack will spin up:

- Autopilot
- Driver
- Baseline

### Fork

- Postgres (with migrations)
- Adminer
- RPC (forked from `reth` or `erigon` node)
- Otterscan (*not yet implemented*)
- Orderbook
- Autopilot
- Driver
- Baseline
- Cow Swap
- Cow Explorer (*not yet implemented*)

### Local

**NOT YET IMPLEMENTED**

- As per fork, but with a local node (not forked from Erigon)
