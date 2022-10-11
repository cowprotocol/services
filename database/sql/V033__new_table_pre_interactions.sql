-- Creates new table for pre_interactions of orders
-- Since one order can have many pre_interactions,
-- the primary key is not the uid, but the whole 
-- pre_interaction

CREATE TABLE pre_interactions (
    order_uid bytea NOT NULL,
    target_to bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    data bytea,
    PRIMARY KEY (order_uid, target_to, value, data)
);

CREATE INDEX uid ON pre_interactions USING HASH (order_uid);


