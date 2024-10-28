-- Change the type of "id" column in proposed_solutions table from bigint to text
ALTER TABLE proposed_solutions
ALTER COLUMN id TYPE numeric(78,0) USING id::numeric(78,0);
