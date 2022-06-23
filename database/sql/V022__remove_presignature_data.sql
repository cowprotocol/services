-- Presignature no longer require signature data to be stored in the database
-- (since the encoded signature bytes is just the order owner which is already
-- stored anyway).
--
-- Remove extrenuous presignature data for:
-- 1. Consistency
-- 2. Disk space (albeit, this is probably very minor in the grand scheme of
--    things).

UPDATE orders SET signature='\x' WHERE signing_scheme='presign';
