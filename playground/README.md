# CoW Playground 

The ultimate goal is to have a single command that will spin up a local CoW network, with all the components needed to run the CoW Protocol stack 🚀

## Quickstart

1. Clone this repo.
2. Change directory to `playground`.
3. Configure the stack by editing the `.env.example` file and renaming it to `.env`. **NOTE**: RPC demand is very high, for optimal performance, use a local node. The stack was tested with `reth` on `mainnet`.
4. Run `docker-compose -f docker-compose.fork.yml up -d`.
5. Configure Rabby Wallet to use the RPC endpoint at `http://localhost:8545` (for `mainnet`, or your network of choice).
6. Configure Rabby Wallet to use a test account (any of the first 10 accounts from the test mnemonic will do).

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

Now with Rabby configured, and the services started, you can browse to http://localhost:8000 and make a trade with CoW Swap. Initially you will start with 10000 ETH, so you will have to wrap some ETH, or alternatively just simply test out an EthFlow order! You can follow along with watching the logs of the `autopilot`, `driver`, and `baseline` solver to see how the Protocol interacts.

## Components

| **Component** | **Container name** | **Host port** | **Container port** | **Stack** |
| --- | --- | --- | --- | --- |
| Autopilot | autopilot | N/A | N/A | Common |
| Driver | driver | N/A | 80 | Common |
| Baseline | baseline | N/A | 80 | Common |
| CoW Swap | cowswap | 8000 | 80 | Local/Fork |
| CoW Explorer | cowexplorer | 8001 | 80 | Local/Fork |
| Orderbook | orderbook | 8080 | 80 | Local/Fork |
| RPC | chain | 8545 | 8545 | Local/Fork |
| Postgres | postgres | 5432 | 5432 | Local/Fork |
| Adminer | adminer | 8082 | 8080 | Local/Fork |

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
