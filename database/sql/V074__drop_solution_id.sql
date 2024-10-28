-- Change the type of "id" column in proposed_solutions table from bigint to text
ALTER TABLE proposed_solutions
ALTER COLUMN id TYPE TEXT USING id::TEXT;
