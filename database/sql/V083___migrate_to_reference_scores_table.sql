-- 1. Create the new reference_scores table
CREATE TABLE reference_scores
(
    auction_id      BIGINT      NOT NULL,
    solver          BYTEA       NOT NULL,
    reference_score NUMERIC(78) NOT NULL,
    PRIMARY KEY (auction_id, solver)
);

-- 2. Migrate data from settlement_scores to reference_scores
INSERT INTO reference_scores (auction_id, solver, reference_score)
SELECT auction_id, winner, reference_score
FROM settlement_scores;

-- 3. Drop the old table
DROP TABLE settlement_scores;
