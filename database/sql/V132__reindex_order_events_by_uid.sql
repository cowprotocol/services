-- `order_events_by_uid` is mostly empty pages left over from the high event
-- volume before the cleanup and event deduplication were introduced. B-tree
-- indexes never return empty pages to the OS, so it has to be rebuilt to
-- reclaim the space.
REINDEX INDEX CONCURRENTLY order_events_by_uid;
