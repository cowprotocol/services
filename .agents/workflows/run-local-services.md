---
description: How to run CoW Protocol services locally (outside Docker playground)
---

# Run CoW Services Locally

Prerequisites: DB + migrations running via `docker compose up` (root docker-compose.yaml), Anvil installed, `cast` and `jq` installed.

// turbo-all

## 1. Start Anvil (forked mainnet)

```bash
anvil --fork-url https://reth-ethereum.ithaca.xyz/rpc --block-time 12
```

## 2. Bypass Allow-List Manager

Run once after Anvil starts:

```bash
cast rpc -r http://127.0.0.1:8545 anvil_setCode 0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE 0x600160005260206000F3
```

## 3. Start Baseline Solver (Terminal 2)

```bash
cargo run --release --bin solvers -- baseline --config configs/manual/baseline.toml --addr 0.0.0.0:9001
```

## 4. Start Driver (Terminal 3)

```bash
TOML_TRACE_ERROR=1 cargo run --release --bin driver -- --config configs/manual/driver.toml --ethrpc http://localhost:8545 --addr 0.0.0.0:11088
```

## 5. Start Orderbook (Terminal 4)

```bash
TOML_TRACE_ERROR=1 cargo run --release --bin orderbook -- \
  --node-url http://localhost:8545 \
  --db-write-url "postgresql://localhost:5432/$USER" \
  --price-estimation-drivers "baseline|http://localhost:11088/baseline" \
  --native-price-estimators "Driver|baseline|http://localhost:11088/baseline" \
  --bind-address 0.0.0.0:8080 \
  --eip1271-skip-creation-validation true \
  --simulation-node-url http://localhost:8545
```

## 6. Start Autopilot (Terminal 5)

```bash
TOML_TRACE_ERROR=1 cargo run --release --bin autopilot -- \
  --config configs/manual/autopilot.toml \
  --node-url http://localhost:8545 \
  --db-write-url "postgresql://localhost:5432/$USER" \
  --price-estimation-drivers "baseline|http://localhost:11088/baseline" \
  --native-price-estimators "Driver|baseline|http://localhost:11088/baseline" \
  --skip-event-sync true \
  --native-price-estimation-results-required 1 \
  --simulation-node-url http://localhost:8545
```

## 7. Place a Test Order

```bash
./scripts/place_order.sh
```

## Notes

- Solver address `0xa0Ee7A142d267C1f36714E4a8F75612F20a79720` = wallet for private key `0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6`
- First `cargo run --release` will be slow (full build). Subsequent runs are fast.
- Pod config is included in driver.toml. Pod network endpoint: `http://cow.pod.network:8545`, auction contract: `0xeDD0670497E00ded712a398563Ea938A29dD28c7`
- Funded pod EOA: `0x903B975112744CE9db137b57A818A907BD35955b`
