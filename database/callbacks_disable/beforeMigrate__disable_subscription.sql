DO $$
DECLARE
  _sub TEXT := '${env}_${network}_alltables';
BEGIN
  IF EXISTS (SELECT 1 FROM pg_subscription WHERE subname = _sub) THEN
     EXECUTE format('ALTER SUBSCRIPTION %I DISABLE;', _sub);
  END IF;
END$$;
