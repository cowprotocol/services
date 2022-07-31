  -- Adding new column on table quotes for the expiration_for_api_call_timestamp
  ALTER TABLE quotes
  ADD COLUMN expiration_for_api_call_timestamp timestamptz;

