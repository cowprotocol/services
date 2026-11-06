-- The documents the orders' app_data hashes are the keccak256 of, uploaded
-- by clients alongside or ahead of their orders.
CREATE TABLE solana.app_data (
    contract_app_data  bytea PRIMARY KEY CHECK (length(contract_app_data) = 32),
    full_app_data      bytea NOT NULL,
    creation_timestamp timestamp with time zone NOT NULL DEFAULT now()
);
