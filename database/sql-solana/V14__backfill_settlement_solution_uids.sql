-- Attribute the settlements already landed to their solution, matched the way
-- the autopilot closes a settlement window: the landed window of the same
-- solver and auction whose solution proposed one of the settlement's trades.
UPDATE solana.settlements s
SET solution_uid = e.solution_uid
FROM solana.settlement_executions e
WHERE s.solution_uid IS NULL
  AND e.auction_id = s.auction_id
  AND e.solver = s.solver
  AND e.outcome = 'landed'
  AND e.submitted_signature = s.tx_signature
  AND EXISTS (
      SELECT 1
      FROM solana.proposed_trade_executions pte
      JOIN solana.trades t
        ON t.tx_signature = s.tx_signature
       AND t.instruction_index = s.instruction_index
       AND t.order_uid = pte.order_uid
      WHERE pte.auction_id = e.auction_id AND pte.solution_uid = e.solution_uid
  );
