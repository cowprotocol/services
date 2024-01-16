ALTER TYPE PolicyKind ADD VALUE 'priceimprovement';

ALTER TABLE fee_policies
    -- quote's sell amount
    ADD COLUMN sell_amount numeric(78,0),
    -- quote's buy amount
    ADD COLUMN buy_amount numeric(78,0);
