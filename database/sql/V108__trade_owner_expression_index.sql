-- Creates an index optimized for searching trades of a given owner.
-- Instead of introducing a whole new owner field that would result
-- in a very expensive table rewrite we use the fact that the uid
-- already contains the owner address.
-- Since postgres supports expression based indexes we can just index
-- the owner slice of the uid.
-- Indexing the `block_number` and `log_index` is needed for postgres
-- to actually use this index in the account trades query.
CREATE INDEX trades_order_uid_owner ON trades (
    substring(order_uid, 33, 20),
    block_number DESC,
    log_index DESC
);
