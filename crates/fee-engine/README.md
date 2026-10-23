# fee-engine

Rule-based protocol fees. A rules file holds named lists, the signers of each
partner and an ordered list of rules. For every order the rules run top to
bottom. The first `when` that is true picks the fee. The fee is then clamped
to the caps in `Settings`.

## Rules file

```toml
[lists]
stablecoins = ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7"]

[partners]
acme = { signers = ["0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"] }

[[rule]]
name = "Stablecoin swaps"
when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
fee  = 'volume(10)'

[[rule]]
name = "Default"
when = 'true'
fee  = 'surplus(50, 1)'
```

`when` and `fee` are [CEL](https://cel.dev) expressions. `when` must return a
bool. `fee` must return one fee or a list of fees. `RuleSet::load` refuses the
whole file when the TOML does not parse, a key is unknown, a rule name repeats,
a partner has no signers, or an expression does not compile. A rule whose
expression fails at run time is logged and skipped.

`tests/rules.toml` is a complete sample file.

## Names bound per auction

`RuleSet::auction` binds the names in this section once per auction. Each
order is then evaluated in a child scope on top of them, which adds the names
of the next section.

### The CEL standard library

The operators (`==`, `&&`, `||`, `in`, `? :`, arithmetic), the standard
functions (`size`, `startsWith`, `endsWith`, `contains`, `matches`,
`timestamp`, `duration`, `string`, `int`) and the macros (`has`, `exists`,
`all`, `filter`, `map`). Optional values are off, so `a.?b` does not compile.

```toml
[[rule]]
name = "Partner fee above 50 bps"
when = '''
  order.appData.appCode.startsWith("CoW Swap")
  && order.appData.metadata.partnerFee.exists(f, has(f.volumeBps) && f.volumeBps > 50)
'''
fee = 'surplus(order.kind == "sell" ? 40 : 50, 1)'

[[rule]]
name = "Orders with pre-hooks"
when = 'size(order.appData.metadata.hooks.pre) > 0'
fee  = 'volume(5)'
```

### Fee functions

The only values a `fee` expression may return. Arguments are CEL expressions,
so a fee can depend on the order. A list stacks several fees.

- `volume(bps)` — `bps / 10000` of the volume.
- `surplus(share, max_volume)` — `share` percent of the surplus over the limit
  price, at most `max_volume` percent of the volume.
- `price_improvement(share, max_volume)` — `share` percent of the improvement
  over the quote, with the same cap.
- `no_fee()` — no protocol fee.

```toml
[[rule]]
name = "Size-dependent volume fee plus price improvement"
when = 'order.partiallyFillable'
fee  = '[volume(size_usd >= 1e6 ? 2 : 5), price_improvement(30, 0.5)]'

[[rule]]
name = "Flashloan orders pay nothing"
when = 'order.appData.metadata.flashloan != null'
fee  = 'no_fee()'
```

### `max` and `min`

Not part of standard CEL, so they are bound by hand. They take several
arguments or one list and compare values of the same type. They clamp a
computed fee inside the rule instead of leaving it to the caps.

```toml
[[rule]]
name = "1 to 10 bps, scaled by size"
when = 'partner == "acme"'
fee  = 'volume(max(1.0, min(10.0, size_usd / 1e5)))'
```

### Lists

Each `[lists]` entry is a CEL list of strings under its name. `in` tests
membership and the list macros work on it. Hex members are lowercased at load
time, as are the hex strings in `order`, so address case does not matter.

```toml
[lists]
stablecoins = ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7"]
jit_makers  = ["0x1111111111111111111111111111111111111111"]

[[rule]]
name = "Stable to stable"
when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
fee  = 'volume(10)'

[[rule]]
name = "JIT maker on either side"
when = 'jit_makers.exists(m, m == order.owner || m == order.receiver)'
fee  = 'no_fee()'
```

### `now`

The auction time as a CEL timestamp, the same for every order in the auction.
It compares with `timestamp("…")` and adds or subtracts a `duration("…")`.
Duration units are `h`, `m`, `s`, `ms`, `us` and `ns`; there is no `d`.

```toml
[[rule]]
name = "Launch month"
when = 'partner == "acme" && now < timestamp("2026-12-01T00:00:00Z")'
fee  = 'no_fee()'

[[rule]]
name = "Orders older than a day"
when = 'now > timestamp(order.creationDate) + duration("24h")'
fee  = 'volume(5)'
```

## Names bound per order

`Auction::evaluate` binds these in a child scope and drops them after the
order.

### `order`

The order as the API returns it, with three changes: `appData` holds the
app-data document instead of its hash, amounts are doubles instead of decimal
strings, and hex strings are lowercase. `appData.metadata` always carries every
protocol field, `null` or empty when unset, with `partnerFee` as a list. An
unset `receiver` is the zero address, not `null`.

```toml
[[rule]]
name = "Large smart-wallet sells"
when = 'order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21'
fee  = 'volume(3)'
```

### `partner`

The app code of a `[partners]` entry whose registered signer attested the
order, or `null`. An order that only claims an `appCode` does not get it.

```toml
[[rule]]
name = "Any verified partner"
when = 'partner != null'
fee  = 'volume(5)'
```

### `size_usd`

The order's volume in USD, as a double. It is bound only once the caller
supplies `Facts::size_usd`. A rule that reads it while it is absent does not
run: `evaluate` returns `Evaluation::Suspended` and the caller evaluates the
order again with the fact loaded.

```toml
[[rule]]
name = "Whale discount"
when = 'size_usd >= 1e6'
fee  = 'volume(1)'
```

## Outcome

`Auction::evaluate` returns one of:

- `Matched { rule, fees }` — the first rule whose `when` held, with each fee
  clamped to `Caps`. A clamp is logged.
- `NoMatch` — no rule matched; the order pays no protocol fee.
- `Suspended { rule, missing }` — `rule` needs a fact that `Facts` did not
  carry.
