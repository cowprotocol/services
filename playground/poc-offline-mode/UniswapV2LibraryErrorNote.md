UniswapV2Library.sol have a hardcoded init code hash in the pairFor function. If you will reset the poc-state.json and deploy everything from scratch, you need to replace the pairFor function initCode, follow the example below:

```sol
    // calculates the CREATE2 address for a pair without making any external calls
    function pairFor(address factory, address tokenA, address tokenB) internal pure returns (address pair) {
        (address token0, address token1) = sortTokens(tokenA, tokenB);
        pair = address(uint(keccak256(abi.encodePacked(
                hex'ff',
                factory,
                keccak256(abi.encodePacked(token0, token1)),
                hex'b6912aa8f91da604bdd903b3484a9f6bb569baa993085fc590133487ff27f91e' // right init code hash
            ))));
    }

```

This can be solved definitely if the UniswapV2Pair deployment generates the same bytecode as the original one (the difference may be caused by forge vs hardhat/truffle deployment), but since this was a PoC, it was made this workaround instead. The current poc-state.json loads the blockchain state with a working UniswapV2Library.sol, so this is not a problem unless you want to install the libs from scratch, start a fresh anvil and deploy everything again.