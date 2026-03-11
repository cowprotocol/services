-- Junction table mapping orders to auctions they participated in.
CREATE TABLE auction_orders (
    auction_id bigint NOT NULL,
    order_uid  bytea  NOT NULL,
    PRIMARY KEY (auction_id, order_uid)
);

CREATE INDEX auction_orders_by_order_uid ON auction_orders (order_uid);
