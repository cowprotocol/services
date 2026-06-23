# Pool-indexer migrations

Flyway migrations for the pool-indexer's own per-network database (e.g.
`ink_pool_indexer`), kept out of the shared `../sql/` set so they don't run
against the autopilot/orderbook main DBs.

The migration image ships both dirs; init containers pick one via `-locations`:

| DB                  | location                                            |
|---------------------|-----------------------------------------------------|
| autopilot/orderbook | `/flyway/sql` (default)                             |
| pool-indexer        | `-locations=filesystem:/flyway/sql-pool-indexer`    |

New pool-indexer migrations go here, never in `../sql/`. `V110` is duplicated
from `../sql/` on purpose: the shared copy can't be deleted (Flyway checksums
applied migrations) so it's cancelled there by `../sql/V111`.
