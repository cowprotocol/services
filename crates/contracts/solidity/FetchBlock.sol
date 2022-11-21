// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title A contract for atomically fetching the latest block state available
/// on a node in a single atomic RPC call.
///
/// This works by doing an `eth_call` to the **pending** block in order to read
/// block data from the latest block (which will be block.number - 1 when
/// executed on the pending block). Note that we can't execute on the latest
/// block, since `blockhash(block.number)` is always 0 and never available.
contract FetchBlock {
    constructor() {
        uint256 blockNumber = block.number > 0 ? block.number - 1 : 0;
        bytes32 blockHash = blockhash(blockNumber);
        bytes32 parentHash = blockNumber > 0
            ? blockhash(blockNumber - 1)
            : bytes32(0);

        bytes memory result = abi.encode(blockNumber, blockHash, parentHash);
        assembly {
            return(add(32, result), mload(result))
        }
    }
}
