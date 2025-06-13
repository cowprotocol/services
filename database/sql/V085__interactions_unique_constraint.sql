-- Include the execution phase into the primary key because there
-- can be multiple interactions with the same index for the same
-- order if they get executed in different phases.
-- Because we are making the primary key stricter than before this
-- modification can not fail.
ALTER TABLE interactions DROP CONSTRAINT interactions_pkey;
ALTER TABLE interactions ADD PRIMARY KEY (order_uid, index, execution);
