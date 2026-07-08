-- with the new index on `competition_auctions::order_uids` we don't need this redundant table anymore
DROP TABLE auction_orders;
