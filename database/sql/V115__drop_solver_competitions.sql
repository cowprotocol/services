-- Drop the solver_competitions table, which was kept only to provide frontend with solver-ids
-- since https://github.com/cowprotocol/cowswap/pull/7871 this is no longer the case
-- and since https://github.com/cowprotocol/services/pull/4657 this has been made effective
-- turning this table obsolete
DROP TABLE IF EXISTS solver_competitions;
