-- Slot of the trade's settlement, copied from `solana.settlements.slot` so
-- trades can be read by slot without a join.
ALTER TABLE solana.trades ADD COLUMN slot bigint;

UPDATE solana.trades AS t
SET slot = s.slot
FROM solana.settlements AS s
WHERE s.tx_signature = t.tx_signature AND s.instruction_index = t.instruction_index;

ALTER TABLE solana.trades ALTER COLUMN slot SET NOT NULL;

CREATE INDEX solana_trades_slot ON solana.trades (slot);
