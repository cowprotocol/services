-- Creates new table for pre_interactions of orders

CREATE TABLE interactions (
    order_uid bytea NOT NULL,
    index int NOT NULL,
    target bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    data bytea NOT NULL,
    PRIMARY KEY(order_uid, index)
);
