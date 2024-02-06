ALTER TYPE PolicyKind ADD VALUE 'priceimprovement';

ALTER TABLE fee_policies
    -- quote's sell amount
    ADD COLUMN quote_sell_amount numeric(78,0),
    -- quote's buy amount
    ADD COLUMN quote_buy_amount numeric(78,0);
