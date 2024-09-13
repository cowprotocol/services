--- We don't need to store the uninternalised and internalised calldata, so this table can be dropped
--- This is because we don't need to show/return the calldata from the competition endpoints for safety reasons
--- The calldata can be recovered using any full node given the transaction hash which is later recovered when observing the settlement
--- See more: https://github.com/cowprotocol/services/issues/2942
DROP TABLE settlement_call_data;
