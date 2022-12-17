# Guidelines

High-level guidelines for writing code in a domain-driven style.

## Tests

Prefer to test at the _highest level of abstraction_. This means that the best tests are those
that interact with the code at the public API layer. E.g., if the code exposes a REST API,
the tests should be hitting the REST API. Mocking should be brought to a minimum. The
only components that should be mocked are those components which are _external_ to our system.
Example: quoting or settlement scoring logic are not external to the system. However, a remote
solver or an external API provider are external to our system.

There are a few reasons why this is a good approach:
- The only way to know that the boundary integrations work (e.g. the boundary between your API
layer and some other layer) is to include that boundary in the tests.
- Code needs to be refactorable. When tests touch implementation details, those implementation
details become harder to change. Furthermore, because tests need to change along with the
implementation details, tests are now useless because you may have made a mistake while changing
them. Developers are less inclined to refactor and code starts to rot.

It's OK to test implementation details if it's an edge case that's impossible to test from the
highest layer, or if it's so difficult to test from the highest layer that testing it that way
becomes impractical.

## Core Logic

This is also referred to as "domain logic". The core logic sets your software
apart from all other software and brings value to your business. For example, almost
every software system today has some form of persistence (DB, writing to files, etc.), some
form of UI (web UI, CLI, REST API), some external components that it consumes, etc. What
sets these software systems apart are the _domain problems_ which they solve. For example, cowswap
is different than any other software system today not because it provides a REST API, but
because it provides a clever way to find the best exchange price for the user across all DEXes.
So when we remove all the externalities and the boilerplate, what we're left with is the _core logic_.
This is the most important part of the codebase. This code needs to be clean and well-documented. This
is the code which defines rules that people who want to understand the system will care about the most.

To this end, when writing code in a domain-driven style, expose a module called `logic` which will
contain the core logic. Model the core logic around _concepts_, not [processes](https://www.martinfowler.com/eaaCatalog/transactionScript.html).
The `driver` crate can serve as a more practical illustration of what this domain-driven architecture
looks like. The `logic` module defines various types like `Auction`, `Solution`, `solution::Score`,
etc. These model the domain logic _concepts_. Other top-level modules such as `api` or `solver`
model boilerplate and external components. (Solvers are external servers which expose an HTTP
interface to the driver.)

## [Data Transfer Objects (DTOs)](https://martinfowler.com/eaaCatalog/dataTransferObject.html)

DTOs generally refer to "models" used to receive or make HTTP or RPC requests, or to store
data in the database, etc. These models should be used _only_ for that one purpose. E.g.,
an API model should be used only for receiving HTTP requests. It should _NEVER_ be
reused in the core logic or in the database storage logic. Other than that, the only other
thing that DTOs should be used for is mapping to/from core logic types.
