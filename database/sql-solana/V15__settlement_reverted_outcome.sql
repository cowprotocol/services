-- `reverted`: the driver saw the settlement land and fail on chain.
ALTER TABLE solana.settlement_executions
    DROP CONSTRAINT settlement_executions_outcome_check,
    ADD CONSTRAINT settlement_executions_outcome_check
        CHECK (outcome IN ('landed', 'rejected', 'reverted', 'timeout'));
