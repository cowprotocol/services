-- Whether a fee policy was configured by the protocol or requested by a
-- partner via the order's app data. NULL for policies stored before this
-- column existed.
CREATE TYPE PolicyOrigin AS ENUM ('protocol', 'partner');

ALTER TABLE fee_policies ADD COLUMN origin PolicyOrigin;
