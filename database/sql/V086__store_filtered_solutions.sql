-- To sanity check the combinatorial auction winner selection
-- we also need to know which solutions got filetered out.
-- Since we only stored non-filtered solutions to begin with
-- we backfill the existing data with `was_filtered = false`.
ALTER TABLE proposed_solutions
    ADD COLUMN was_filtered BOOLEAN NOT NULL DEFAULT FALSE;

-- Drop default because we want to enforce this data to be set.
ALTER TABLE proposed_solutions
    ALTER COLUMN was_filtered DROP DEFAULT;
