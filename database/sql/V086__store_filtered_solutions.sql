-- To sanity check the combinatorial auction winner selection
-- we also need to know which solutions got filtered out.
-- Since we only stored solutions that passed the filter to
-- begin with we backfill the existing data with
-- `filtered_out = false`.
ALTER TABLE proposed_solutions
    ADD COLUMN filtered_out BOOLEAN NOT NULL DEFAULT FALSE;

-- Drop default because we want to enforce this data to be set.
ALTER TABLE proposed_solutions
    ALTER COLUMN filtered_out DROP DEFAULT;
