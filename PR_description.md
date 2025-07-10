# Test PR: Add timestamp column to app_data table

This PR adds a `created_at` timestamp column to the `app_data` table to test flyway migrations.

## Changes

- **Migration**: Added `V088__add_app_data_timestamp.sql` with:
  - New `created_at` column with `TIMESTAMP WITH TIME ZONE` type
  - Default value of `CURRENT_TIMESTAMP` for existing and new records

## Purpose

This is a test migration to verify that:
1. Flyway migrations work correctly in our deployment pipeline
2. Database schema changes can be applied safely
3. The migration rollback process works as expected

## Migration Details

```sql
ALTER TABLE app_data 
ADD COLUMN created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL;
```

## Impact

- **Non-breaking**: The migration only adds a new column with a default value
- **Backward compatible**: Existing application code will continue to work
- **Performance**: Minimal impact as the column has a default value
- **Storage**: Small increase in table size due to new timestamp column

## Testing

This migration can be tested on staging environments before production deployment to ensure:
- Migration applies successfully
- No performance degradation
- Application continues to function normally
- Rollback works if needed 