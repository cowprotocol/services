DO $$
DECLARE
  _sub TEXT := '${env}_${network}_alltables';
BEGIN
  IF EXISTS (SELECT 1 FROM pg_subscription WHERE subname = _sub) THEN
     EXECUTE format('ALTER SUBSCRIPTION %I ENABLE;',  _sub);
     PERFORM pg_sleep(3);          -- wait for worker restart (optional but safe)
     EXECUTE format('ALTER SUBSCRIPTION %I REFRESH PUBLICATION;', _sub);
  END IF;
END$$;
