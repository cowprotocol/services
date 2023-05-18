CREATE TYPE ExecutionTime AS ENUM ('pre', 'post');
-- Add new column specifying when the interaction is supposed to be executed.
-- We only set `pre` as the default temporarily because this will cause all rows to get the correct value immediately
-- (currently all interactions are pre-interactions).
ALTER TABLE interactions ADD COLUMN execution ExecutionTime NOT NULL DEFAULT 'pre';
-- But afterwards we drop the default again because we want to make sure that all interactions specify when they
-- are supposed to be executed.
ALTER TABLE interactions ALTER COLUMN execution DROP DEFAULT;
