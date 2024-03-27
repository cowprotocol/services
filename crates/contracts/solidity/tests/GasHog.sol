// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

interface ERC20 {
    function approve(address spender, uint amount) external;
}

/// @title Helper contract to simulate gas intensive ERC1271 signatures
contract GasHog {
    function isValidSignature(bytes32 order, bytes calldata signature) public view returns (bytes4) {
        uint start = gasleft();
        uint target = abi.decode(signature, (uint));
        bytes32 hash = keccak256("go");
        while (start - gasleft() < target) {
            hash = keccak256(abi.encode(hash));
        }
        // Assert the impossible so that the compiler doesn't optimise the loop away
        require(hash != order);

        // ERC1271 Magic Value
        return 0x1626ba7e;
    }

    function approve(ERC20 token, address spender, uint amount) external {
        token.approve(spender, amount);
    }
}
