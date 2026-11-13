-- The protocol fee policies of the orders ranked solutions trade, in the
-- EVM fee_policies columns. A price improvement policy measures against the
-- quote solana.order_quotes links the order to.
CREATE TYPE solana.PolicyKind AS ENUM ('surplus', 'volume', 'priceimprovement');

CREATE TABLE solana.fee_policies (
    auction_id        bigint NOT NULL,
    order_uid         bytea  NOT NULL CHECK (length(order_uid) = 32),
    -- The order the policies apply in.
    application_order bigint GENERATED ALWAYS AS IDENTITY,
    kind              solana.PolicyKind NOT NULL,
    -- Factors are in [0, 1): the share of the surplus, or of the price
    -- improvement over the quote, capped at the max volume share of the
    -- order's volume, or a plain share of the volume.
    surplus_factor                      double precision,
    surplus_max_volume_factor           double precision,
    price_improvement_factor            double precision,
    price_improvement_max_volume_factor double precision,
    volume_factor                       double precision,
    PRIMARY KEY (auction_id, order_uid, application_order)
);
