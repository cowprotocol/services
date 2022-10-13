-- Creates new table for pre_interactions of orders

CREATE TABLE pre_interactions (
    order_uid bytea NOT NULL,
    target_to bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    data bytea NOT NULL,
);

CREATE INDEX uid ON pre_interactions USING HASH (order_uid);
