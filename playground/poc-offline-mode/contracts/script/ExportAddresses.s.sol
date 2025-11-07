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
        address uniswapFactory = vm.envAddress("UNISWAP_FACTORY");
        address uniswapRouter = vm.envAddress("UNISWAP_ROUTER");
        address pairWethUsdc = vm.envAddress("PAIR_WETH_USDC");
        address pairWethDai = vm.envAddress("PAIR_WETH_DAI");
        address pairUsdcDai = vm.envAddress("PAIR_USDC_DAI");
        address cowAuthenticator = vm.envAddress("COW_AUTHENTICATOR");
        address cowVaultRelayer = vm.envAddress("COW_VAULT_RELAYER");
        address cowSettlement = vm.envAddress("COW_SETTLEMENT");
        address balancerVault = vm.envAddress("BALANCER_VAULT");
        
        console.log("Exporting addresses to JSON...");
        console.log("");
        
        // Build JSON object
        string memory json = string(abi.encodePacked(
            '{\n',
            '  "chainId": "31337",\n',
            '  "tokens": {\n',
            '    "WETH": "', vm.toString(weth), '",\n',
            '    "USDC": "', vm.toString(usdc), '",\n',
            '    "DAI": "', vm.toString(dai), '"\n',
            '  },\n',
            '  "uniswapV2": {\n',
            '    "factory": "', vm.toString(uniswapFactory), '",\n',
            '    "router": "', vm.toString(uniswapRouter), '",\n',
            '    "pairs": {\n',
            '      "WETH-USDC": "', vm.toString(pairWethUsdc), '",\n',
            '      "WETH-DAI": "', vm.toString(pairWethDai), '",\n',
            '      "USDC-DAI": "', vm.toString(pairUsdcDai), '"\n',
            '    }\n',
            '  },\n',
            '  "cowProtocol": {\n',
            '    "settlement": "', vm.toString(cowSettlement), '",\n',
            '    "authenticator": "', vm.toString(cowAuthenticator), '",\n',
            '    "vaultRelayer": "', vm.toString(cowVaultRelayer), '",\n',
            '    "balancerVault": "', vm.toString(balancerVault), '"\n',
            '  }\n',
            '}\n'
        ));
        
        // Write to file
        vm.writeFile("./config/addresses.json", json);
        
        console.log("Addresses exported to ./config/addresses.json");
        console.log("");
        console.log(json);
    }
}
