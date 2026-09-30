-- Per-order penalty caps (CIP-87): the maximum penalty a solver can incur for
-- winning an order but failing to execute it, in lamports. Mapped one-to-one
-- with `order_uids`.
ALTER TABLE solana.competition_auctions
    ADD COLUMN penalty_caps_native numeric(20,0)[]
    GENERATED ALWAYS AS (array_fill(0::numeric(20,0), ARRAY[cardinality(order_uids)])) STORED;
