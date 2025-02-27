-- Improve performance for queries that filter by time range
CREATE INDEX settlement_executions_time_range_index ON settlement_executions (start_timestamp, end_timestamp);
