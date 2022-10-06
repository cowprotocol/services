-- Per order solver reward for https://forum.cow.fi/t/cip-draft-risk-adjusted-solver-rewards/1132 .
-- Used in the weekly solver payout.
CREATE TABLE order_rewards (
    order_uid bytea NOT NULL,
    auction_id bigint NOT NULL,
    reward double precision NOT NULL,
    PRIMARY KEY(order_uid, auction_id)
);
