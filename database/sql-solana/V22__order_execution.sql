-- The protocol fees an order paid in a landed settlement, one row per order
-- and auction, in the EVM order_execution columns. Each fee is in the
-- order's surplus token, and the arrays follow the order's
-- solana.fee_policies rows by application_order. They are empty when the
-- order had no policies or its fees did not compute.
CREATE TABLE solana.order_execution (
    order_uid            bytea           NOT NULL CHECK (length(order_uid) = 32),
    auction_id           bigint          NOT NULL,
    protocol_fee_tokens  bytea[]         NOT NULL DEFAULT '{}',
    protocol_fee_amounts numeric(20,0)[] NOT NULL DEFAULT '{}',
    PRIMARY KEY (order_uid, auction_id)
);
