module.exports = {
    networks: {
        hardhat: {
            initialBaseFeePerGas: 0,
            initialDate: "2000-01-01T00:00:00.000+00:00",
            accounts: {
                accountsBalance: "1000000000000000000000000"
            },
            gas: 1e7,
            gasPrice: 1
        }
    }
};
