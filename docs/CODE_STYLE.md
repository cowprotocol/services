# Code Style Guide

Rules for generating and editing code in this repo. Follow these before making non-trivial changes.

## 1. Comments explain WHY, not WHAT — and never the conversation

Comments exist for a future reader who has **only the code** — not this chat, not the PR description, not the task tracker.

**Write a comment when** the reason for the code is non-obvious from reading it:
- A domain constraint that justifies the shape of the code
- A subtle invariant the reader must not break
- A workaround for an external bug (link the issue)
- Async ordering, cancellation, or lifetime subtleties that would surprise a reader

**Do not write a comment that:**
- Restates what a well-named function or variable already says
- References the current task, PR, or user request (`// added for the X flow`, `// per user's request`, `// handles the case we discussed`)
- Names callers (`// used by autopilot when …`) — grep exists and these rot
- Recaps chat context or planning discussion
- Explains implementation details that belong in the identifier's name instead
- Marks removed code (`// removed foo()`) — just delete it
- States how new code differs from the old one - we only need to understand the current code

Litmus test: if removing the comment would not confuse a reader who has never seen this conversation, delete it.

Docstrings on public items describe the contract — inputs, outputs, invariants, error conditions. Not narrative history.

## 2. Bundle related logic into components

When the same set of values keeps getting threaded into several free-standing functions, that is a design smell. Group them into a struct and expose the operations as methods (or behind a trait).

Benefits:
- Ownership of state is explicit
- Signatures shrink; call sites read better
- Invariants can be established once at construction and relied on thereafter

Smell:
```rust
fn compute_price(pool: &Pool, token_in: &Token, token_out: &Token, amount: U256) -> U256 { ... }
fn compute_fee(pool: &Pool, token_in: &Token, token_out: &Token, amount: U256) -> U256 { ... }
fn simulate(pool: &Pool, token_in: &Token, token_out: &Token, amount: U256) -> Result<Trace> { ... }
```

Better:
```rust
struct Swap<'a> {
    pool: &'a Pool,
    token_in: &'a Token,
    token_out: &'a Token,
    amount: U256,
}

impl Swap<'_> {
    fn price(&self) -> U256 { ... }
    fn fee(&self) -> U256 { ... }
    fn simulate(&self) -> Result<Trace> { ... }
}
```

Trigger: *repetition of the same argument set*, not just function count. Do not manufacture a struct for a single caller with one method.

## 3. Prefer early returns to deep nesting

Guard clauses and `?` beat pyramids of `if` / `if let` / nested `match`. The happy path should stay on the left margin.
This eases burden on readers since they can build an incremental view of the state as they read the code.

Avoid:
```rust
fn handle(order: Order) -> Result<Filled> {
    if let Some(quote) = order.quote {
        if quote.is_valid() {
            match order.kind {
                Kind::Buy => {
                    // ...
                    Ok(filled)
                }
                Kind::Sell => { /* ... */ }
            }
        } else {
            Err(Error::InvalidQuote)
        }
    } else {
        Err(Error::MissingQuote)
    }
}
```

Prefer:
```rust
fn handle(order: Order) -> Result<Filled> {
    let quote = order.quote.ok_or(Error::MissingQuote)?;
    if !quote.is_valid() {
        return Err(Error::InvalidQuote);
    }
    match order.kind {
        Kind::Buy => { /* ... */ }
        Kind::Sell => { /* ... */ }
    }
}
```

A single `match` on an enum is not "nesting" in the bad sense — it gives exhaustiveness. The problem is stacked `if` / `if let` pyramids that push the real logic off the right edge. Flatten only when it does not hurt readability; do not flatten at the cost of losing exhaustive matching on an enum.

## 4. Don't change code or comments unnecessarily

Humans have to review each PR so let's not waste their time with reviewing unnecessary changes.

