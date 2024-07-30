ALTER TABLE quotes ADD COLUMN creation_timestamp timestamptz NOT NULL DEFAULT now();
