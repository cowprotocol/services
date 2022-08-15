-- When getting orders by tx hash we index the settlements table by tx_hash.
CREATE INDEX settlements_tx_hash ON settlements USING HASH (tx_hash);
