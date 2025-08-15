#!/bin/bash

set -o errexit -o pipefail -o nounset

repo_root_dir="$(git rev-parse --show-toplevel)"

for deployment in "$repo_root_dir/crates/contracts/broadcast/"*"/"*"/"*".json"; do
  # The subfolder name is the chain id
  chain_id=${deployment%/*}
  chain_id=${chain_id##*/}

  # First, every single deployment is formatted as if it had its own networks.json
  jq --arg chainId "$chain_id" '
    .transactions[]
    | select(.transactionType == "CREATE2")
    | select(.hash != null)
    | {(.contractName): {($chainId): {address: .contractAddress, transactionHash: .hash }}}
  '  <"$deployment"
done \
  | # Then, all these single-contract single-chain-id networks.jsons are merged. Note: in case the same contract is
    # deployed twice in the same script run, the last deployed contract takes priority.
    jq --sort-keys --null-input 'reduce inputs as $item ({}; . *= $item)'
