-- Wait-for-CoW orders only trade in canonical batches. The autopilot stores a
-- `filtered` order event with this reason for the ones it leaves out of an
-- auction, and the orderbook reports such orders as `scheduled`.
ALTER TYPE OrderFilterReason ADD VALUE 'waiting_for_canonical_batch';
