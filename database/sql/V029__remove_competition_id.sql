-- We no longer need an auto incrementing id because we use the auction id.

ALTER TABLE solver_competitions
    ALTER COLUMN id DROP DEFAULT
;
DROP SEQUENCE solver_competitions_id_seq;
