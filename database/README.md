This document explains the database that the orderbook api uses.

We need to store orders persistently even if the orderbook service goes. For this we use a PostgreSQL database. The schema can be found in `schema.sql`.

In addition to the orders, we also store trade events. This allows us to keep track of filled amounts.

To prevent users from spamming the database we should when receiving an order check that
* the erc20 spending is approved
* valid_to is not in the past and not more than <1 year> in the future
* the order owner does not already have more than <1000> valid orders

This ensures that the amount of orders we have to keep track of per user is limited and that there is a real cost the user incurs before being able to submit orders because they need to have sent an ethereum transaction.

Technically, this could still be attacked with a fake token that claims that approval has always been granted. This is more evidence to my belief that we should whitelist erc20 tokens that we will handle.
