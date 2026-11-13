-- The wallet `buy_token_account` is the associated token account of, when
-- that is not the owner. The settlement creates a missing account at that
-- address for this wallet, like the owner's own. NULL means the owner.
ALTER TABLE solana.orders
    ADD COLUMN receiver bytea CHECK (receiver IS NULL OR length(receiver) = 32);
