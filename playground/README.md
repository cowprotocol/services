# CoW Playground

The ultimate goal is to have a single command that will spin up a local CoW network, with all the components needed to
run the CoW Protocol stack. The stack will also auto-recompile and restart any services for a live editing experience 🚀

## Quickstart

1. Clone this repo.
2. Configure the stack by editing the `.env.example` file and renaming it to `.env`. **NOTE**: RPC demand is very high,
   for optimal performance, use a local node. The stack was tested with `reth` on `mainnet`.
3. Run `docker-compose -f docker-compose.fork.yml up --build`.
    * If you're *not* using Linux, we recommend you use the `docker-compose.non-interactive.yml` definition instead;
      due to how mounts are done outside of Linux, running cargo-watch leads to [very slow buids](https://github.com/watchexec/cargo-watch#docker-running-cargo-commands-over-a-mount-is-very-slow).
4. Configure Rabby Wallet (or see [Metamask specific notes](#metamask)) to use the RPC endpoint at
   `http://localhost:8545` (for `mainnet`, or your network of choice).
5. Configure your wallet to use a test account (any of the first 10 accounts from the test mnemonic will do).

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
> If you make any changes to the files in your repo directory, services will automatically be recompiled and restarted.
> The CoW Explorer is available at http://localhost:8001 to see more information about transaction status

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

| **Component** | **Container name** | **Host port** | **Container port** | **Tokio Console Port** | **Stack**  |
|---------------|--------------------|---------------|--------------------|------------------------|------------|
| Autopilot     | autopilot          | N/A           | N/A                | 6670                   | Local/Fork |
| Driver        | driver             | N/A           | 80                 | 6671                   | Local/Fork |
| Baseline      | baseline           | N/A           | 80                 | 6672                   | Local/Fork |
| CoW Swap      | cowswap            | 8000          | 80                 | N/A                    | Local/Fork |
| CoW Explorer  | cowexplorer        | 8001          | 80                 | N/A                    | Local/Fork |
| Orderbook     | orderbook          | 8080          | 80                 | 6669                   | Local/Fork |
| RPC           | chain              | 8545          | 8545               | N/A                    | Local/Fork |
| Postgres      | postgres           | 5432          | 5432               | N/A                    | Local/Fork |
| Adminer       | adminer            | 8082          | 8080               | N/A                    | Local/Fork |
| Grafana       | grafana            | 3000          | 3000               | N/A                    | Local/Fork |
| Otterscan     | otterscan          | 8003          | 80                 | N/A                    | Local/Fork |
| Sourcify      | sourcify           | 5555          | 5555               | N/A                    | Local/Fork |
| Sourcify DB   | sourcify-db        | N/A           | 5432               | N/A                    | Local/Fork |

**NOTE**: Currently only **FORK** mode is supported.

Some services support to be inspected with [tokio-console](https://github.com/tokio-rs/console). For that simply install `tokio-console` and run `tokio-console http://localhost:<PORT>`. The relevant port numbers can be found in the table above.

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
- Otterscan
- Sourcify (contract verification)
- Orderbook
- Autopilot
- Driver
- Baseline
- Cow Swap
- Cow Explorer

### Local

**NOT YET IMPLEMENTED**

- As per fork, but with a local node (not forked from Erigon)

## Using Otterscan

[Otterscan](https://github.com/otterscan/otterscan) is a local block explorer integrated into the playground. It provides powerful transaction analysis tools that work with your local Anvil chain — no external services required.

**Access Otterscan at:** http://localhost:8003

### Inspecting Transactions

When you make a swap in CoW Swap, all explorer links automatically point to your local Otterscan instance. You can:

1. **View transaction overview**: See gas usage, status, block info, and decoded input data
2. **Analyze transaction traces**: Expand the full call tree to see every internal call, including:
   - Contract-to-contract calls
   - Token transfers (ERC20, ERC721)
   - Value transfers
   - Delegate calls and static calls
3. **View event logs**: See all emitted events with decoded parameters
4. **Check gas profiling**: Understand gas consumption per operation

### Debugging Failed Transactions

Otterscan is especially useful for debugging failed transactions:

1. Navigate to the failed transaction in Otterscan
2. Check the **"Trace"** tab to see exactly where the transaction reverted
3. Look for the revert reason in the trace output (e.g., `Error(string)` or custom errors)
4. For CoW Protocol transactions, you can trace the entire settlement flow including:
   - Pre-interactions
   - Token approvals and transfers
   - AMM interactions (Uniswap, etc.)
   - Post-interactions

### Example: Tracing a CoW Swap Settlement

After executing a swap:

1. Click the transaction link in the CoW Swap UI or CoW Explorer. This will open the transaction directly in your local Otterscan instance.
2. Click on **"Trace"** to see the full execution flow.
3. Expand the `settle()` call to see:
   - How tokens flowed between parties
   - Which liquidity sources were used
   - Gas costs per operation

> **Tip:** For details on how Otterscan displays verified source code, see the [Contract Verification with Sourcify](#contract-verification-with-sourcify) section below.

## Contract Verification with Sourcify

The playground includes a local [Sourcify](https://sourcify.dev/) instance for contract verification. Sourcify is a decentralized contract verification service that matches deployed bytecode with source code. Verified contracts display their source code in Otterscan.

**How it works:**

- **Cloud mode** (`SOURCIFY_MODE=cloud`): Otterscan fetches verified source code from the public Sourcify repository. This shows source code for well-known contracts (CoW Protocol, USDC, etc.) that have been publicly verified.
- **Local mode** (`SOURCIFY_MODE=local`): Otterscan fetches from your local Sourcify instance. Use this when testing contracts you deploy and verify locally.

### Sourcify Sources Configuration

Configure which Sourcify source Otterscan uses in your `.env` file:

```bash
# Use public Sourcify (default) - shows publicly verified contracts
SOURCIFY_MODE=cloud

# Use local Sourcify - shows contracts verified on your local instance
SOURCIFY_MODE=local
```

After changing this value, recreate the Otterscan container:

```bash
docker compose -f docker-compose.fork.yml up -d otterscan
```

or

```bash
docker compose -f docker-compose.non-interactive.yml up -d otterscan
```

> **Note**: A simple `docker compose restart` won't work because it doesn't re-read `.env` - you need to recreate the container.

### Verifying Contracts

You can verify contracts on the local Sourcify instance using:

1. **Sourcify API**: POST to `http://localhost:5555/verify` with your contract address, chain ID, and source files
2. **Foundry**: Use `forge verify-contract` with `--verifier sourcify --verifier-url http://localhost:5555`

After verification, view the contract source in Otterscan at `http://localhost:8003/address/<contract_address>`.
