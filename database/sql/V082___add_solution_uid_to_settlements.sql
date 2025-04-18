-- 1. Add a new nullable column that corresponds to the winning solution provided by the solver
ALTER TABLE settlements ADD COLUMN solution_uid bigint;

-- 2. Backfill using only winning solutions. Currently, only a single solution per solver can be winning.
UPDATE settlements s
SET solution_uid = ps.uid
FROM proposed_solutions ps
WHERE s.auction_id IS NOT NULL
  AND s.auction_id = ps.auction_id
  AND s.solver = ps.solver
  AND ps.is_winner = TRUE;
