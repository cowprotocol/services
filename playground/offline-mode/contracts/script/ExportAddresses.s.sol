// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Script.sol";

/// @title ExportAddresses
/// @notice Export all deployed contract addresses to JSON format
contract ExportAddresses is Script {
    function run() external {
        // Load addresses from environment (written by previous deployment scripts)
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");
        address usdt = vm.envAddress("USDT_ADDRESS");
        address gno = vm.envAddress("GNO_ADDRESS");
        address uniswapFactory = vm.envAddress("UNISWAP_FACTORY");
        address uniswapRouter = vm.envAddress("UNISWAP_ROUTER");
        address pairWethUsdc = vm.envAddress("PAIR_WETH_USDC");
        address pairWethDai = vm.envAddress("PAIR_WETH_DAI");
        address pairWethUsdt = vm.envAddress("PAIR_WETH_USDT");
        address pairWethGno = vm.envAddress("PAIR_WETH_GNO");
        address pairUsdcDai = vm.envAddress("PAIR_USDC_DAI");
        address pairUsdcUsdt = vm.envAddress("PAIR_USDC_USDT");
        address pairUsdcGno = vm.envAddress("PAIR_USDC_GNO");
        address pairDaiUsdt = vm.envAddress("PAIR_DAI_USDT");
        address pairDaiGno = vm.envAddress("PAIR_DAI_GNO");
        address pairUsdtGno = vm.envAddress("PAIR_USDT_GNO");
        address cowAuthenticator = vm.envAddress("COW_AUTHENTICATOR");
        address cowVaultRelayer = vm.envAddress("COW_VAULT_RELAYER");
        address cowSettlement = vm.envAddress("COW_SETTLEMENT");
        address balancerVault = vm.envAddress("BALANCER_VAULT");
        
        console.log("Exporting addresses to JSON...");
        console.log("");
        
        // Build JSON object (split into parts to avoid stack too deep)
        string memory tokensJson = string(abi.encodePacked(
            '  "tokens": {\n',
            '    "WETH": "', vm.toString(weth), '",\n',
            '    "USDC": "', vm.toString(usdc), '",\n',
            '    "DAI": "', vm.toString(dai), '",\n',
            '    "USDT": "', vm.toString(usdt), '",\n',
            '    "GNO": "', vm.toString(gno), '"\n',
            '  }'
        ));

        // Split pairs into multiple parts to avoid stack too deep
        string memory pairsJson1 = string(abi.encodePacked(
            '      "WETH-USDC": "', vm.toString(pairWethUsdc), '",\n',
            '      "WETH-DAI": "', vm.toString(pairWethDai), '",\n',
            '      "WETH-USDT": "', vm.toString(pairWethUsdt), '",\n',
            '      "WETH-GNO": "', vm.toString(pairWethGno), '",\n'
        ));

        string memory pairsJson2 = string(abi.encodePacked(
            '      "USDC-DAI": "', vm.toString(pairUsdcDai), '",\n',
            '      "USDC-USDT": "', vm.toString(pairUsdcUsdt), '",\n',
            '      "USDC-GNO": "', vm.toString(pairUsdcGno), '",\n'
        ));

        string memory pairsJson3 = string(abi.encodePacked(
            '      "DAI-USDT": "', vm.toString(pairDaiUsdt), '",\n',
            '      "DAI-GNO": "', vm.toString(pairDaiGno), '",\n',
            '      "USDT-GNO": "', vm.toString(pairUsdtGno), '"\n'
        ));

        string memory pairsJson = string(abi.encodePacked(pairsJson1, pairsJson2, pairsJson3));

        string memory uniswapJson = string(abi.encodePacked(
            '  "uniswapV2": {\n',
            '    "factory": "', vm.toString(uniswapFactory), '",\n',
            '    "router": "', vm.toString(uniswapRouter), '",\n',
            '    "pairs": {\n',
            pairsJson,
            '    }\n',
            '  }'
        ));

        string memory cowJson = string(abi.encodePacked(
            '  "cowProtocol": {\n',
            '    "settlement": "', vm.toString(cowSettlement), '",\n',
            '    "authenticator": "', vm.toString(cowAuthenticator), '",\n',
            '    "vaultRelayer": "', vm.toString(cowVaultRelayer), '",\n',
            '    "balancerVault": "', vm.toString(balancerVault), '"\n',
            '  }'
        ));

        string memory json = string(abi.encodePacked(
            '{\n',
            '  "chainId": "31337",\n',
            tokensJson,
            ',\n',
            uniswapJson,
            ',\n',
            cowJson,
            '\n}\n'
        ));
        
        // Write to file
        vm.writeFile("./config/addresses.json", json);
        
        console.log("Addresses exported to ./config/addresses.json");
        console.log("");
        console.log(json);
    }
}
