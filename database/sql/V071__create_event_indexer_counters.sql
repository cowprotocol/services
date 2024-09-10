CREATE TABLE event_indexer_counters
(
    name    VARCHAR(255) PRIMARY KEY,
    counter BIGINT NOT NULL DEFAULT 0
);

INSERT INTO event_indexer_counters (name, counter)
VALUES ('gpv2_settlement_indexer',
        GREATEST(
                (SELECT COALESCE(MAX(block_number), 0) FROM trades),
                (SELECT COALESCE(MAX(block_number), 0) FROM settlements),
                (SELECT COALESCE(MAX(block_number), 0) FROM invalidations),
                (SELECT COALESCE(MAX(block_number), 0) FROM presignature_events)
        ));

INSERT INTO event_indexer_counters (name, counter)
VALUES ('ethflow_refund_indexer', (SELECT COALESCE(MAX(block_number), 0) FROM ethflow_refunds));

INSERT INTO event_indexer_counters (name, counter)
VALUES ('onchain_order_indexer', (SELECT COALESCE(MAX(block_number), 0) FROM onchain_placed_orders));
