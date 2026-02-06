-- composite index to speed up query for user orders with quote
CREATE INDEX CONCURRENTLY orders_owner_class_valid_composite ON orders (owner, class, true_valid_to DESC);
