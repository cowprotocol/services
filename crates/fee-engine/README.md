# fee-engine

Rule-based protocol fees. A `RuleSet` holds named lists and an ordered list
of rules. For every order every rule runs; the engine reports each rule whose
`when` is true, in rule order, with its fee. The caller picks which matches
to act on, e.g. the first. The partner fees of the order's app data are not
rules: the engine returns them apart, capped, to follow the picked fees.

## Rules

A rule is a `RuleDefinition`: a name and two expressions in
[CEL](https://cel.dev) syntax. `when` must return a bool. `fee` must return
one fee or a list of fees. The engine does not run a CEL interpreter: the
`cel` crate only parses, and `RuleSet::new` compiles each expression to a
typed Rust closure (`src/compile`). It supports the subset of CEL listed
under [Names a rule can use](#names-a-rule-can-use); anything else does not
compile.

`RuleSet::new` refuses the whole set when a rule name is empty or repeats,
a list takes a reserved name or an expression does not compile. An
expression does not compile when it does not parse, names an unknown field,
list or function, mixes types (`order.sellAmount == "x"`, a `when` that is
not a bool), or has a constant part that fails, such as a fee argument off
the grid (`volume(0.001)`), in any branch. A rule whose expression fails on
an order (a missing optional field, an arithmetic overflow, a computed fee
argument off the grid) is skipped for that order and reported to the
caller.

`RuleSet::validate_on_sample_order` is a separate pass over two sample
orders, so one run shows all the mistakes that compiling cannot find. On
the full sample (every optional field present) both expressions of every
rule run, whatever `when` says, and every one that fails is reported. On
the bare sample (no optional field present) the rules run as on a real
order, and every rule that is skipped is reported. This catches a rule that
reads a field an order may lack, such as `order.ethflowData.userValidTo`,
directly instead of through the optional operator
(`order.?ethflowData.userValidTo`), which would be skipped on every order
that is not an ethflow order.

The pass has limits:

- A ternary, `&&` or `||` runs only what the sample order reaches.
- Every amount in the samples is 1, and every timestamp is `now`. A fee
  computed from the order is checked only at those values:
  `volume(order.buyAmount / order.sellAmount)` passes, but on a real order
  it can give an out-of-range argument, and the rule is then skipped and
  reported.

```
name: Stablecoin swaps
when: order.sellToken in stablecoins && order.buyToken in stablecoins
fee:  volume(10)

name: Default
when: true
fee:  surplus(50, 100)
```

## The rules file

`load_rules_from_str(source)` reads a `RuleSetDefinition` from TOML:
`[lists]` holds named lists of strings, each `[[rule]]` table is a rule,
tried in file order, and `[partner-fees]` holds `max-volume-bps`, the basis
points of the volume an order's partner fees may take together; it lands on
the fee grid like a fee argument. Without the table the engine returns no
partner fee, as the orderbook counts none without
`order-quoting.max-partner-fee`.
Unknown top-level and rule keys are refused; any key under `[lists]` is a
list name, so a list that no rule names loads without error. The file goes
through `RuleSet::new` and `RuleSet::validate_on_sample_order`; any failure
refuses the whole file as a `LoadError`: `Toml` (does not parse, unknown
key, missing field, a rule field that is empty or a bad `max-volume-bps`;
names the line and column), `Rules` (a `CompileError`) or `SampleOrder` (every
`SampleOrderError`). A file without rules loads: no order pays a protocol
fee.

`load_rules_toml(RulesToml { lists, rules, partner_fees })` loads the same
tables as raw `toml::Value`s, e.g. from a section of a host config, so the
host need not know their shape. `None` is an absent table, as in a file. It
refuses them as `load_rules_from_str` does, but without a line or column: a
malformed rule is a `MalformedRule` error that names the rule by its number,
and any other malformed value is a `Toml` error.

```toml
[partner-fees]
max-volume-bps = 100

[lists]
stablecoins = [
  "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", # USDC
  "0xdAC17F958D2ee523a2206206994597C13D831ec7", # USDT
]
jit_makers = ["0x1111111111111111111111111111111111111111"]

[[rule]]
name = "JIT makers"
when = 'order.owner in jit_makers'
fee  = 'no_fee()'

[[rule]]
name = "Trust Wallet"
when = 'order.appData.?appCode.orValue("").lowerAscii() == "trust wallet"'
fee  = 'no_fee()'

[[rule]]
name = "Stablecoin swaps"
when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
fee  = 'volume(10)'

[[rule]]
name = "Default"
when = 'true'
fee  = 'surplus(50, 100)'
```

`tests/mainnet.toml` is the mainnet policy as rules, with short token
lists. A fee decision is identified by `Match::rule.name()`. To resolve that
name to the rule text in git, the host must log which rules file it loaded.

## Names a rule can use

`RuleSet::evaluator` fixes `now`; `Evaluator::evaluate` reads `order` from
the `OrderView` the host passes. The lists and every constant part of an
expression are bound when the rules compile.

### Operators and functions

- Literals: bools, numbers (`2`, `2u`, `0.3`, `1e24`), strings and lists of
  fees or of strings.
- `&&`, `||`, `!`, `? :`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `in`.
- `+ - * / %` on numbers; `+` and `-` on timestamps and durations.
- `has(...)`, and the optional operator `.?` ended by `.orValue(d)` or
  `.hasValue()`; see the `order` section.
- `timestamp`, `duration`, `int`, `uint`, `double` (a decimal, see
  [Numbers](#numbers)).
- `lowerAscii`, `upperAscii`, `startsWith`, `endsWith`, `contains` on
  strings.
- `math.greatest`, `math.least`, `math.ceil`, `math.floor`, `math.round`
  (half away from zero), `math.trunc`, `math.abs`.

Not supported: macros (`exists`, `all`, `filter`, `map`), maps, indexing,
`size`, `matches`, `string`, `optMap`, `math.sqrt` and the rest of the CEL
extensions. A rule that uses one does not compile; test a list with `in`.

```
when: order.kind == "sell" && order.sellAmount > 1e21
fee:  surplus(order.partiallyFillable ? 40 : 50, 100)
fee:  volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e20)))
```

### Numbers

Numbers are exact; there are no doubles.

- `uint` is 256 bits: `sellAmount`, `buyAmount`, `validTo`, `validFrom`.
- `int` is 256 bits: `ethflowData.userValidTo` and integer literals.
- A number with a fraction is a decimal (`rust_decimal`, 28 digits), as are
  literals such as `0.3` and `1e21`. The engine reads each literal from the
  rule text, so `1e24` is exactly 10^24.

Values of different types compare exactly. In arithmetic, a constant takes
the type of the other side when its value does not change, so
`order.validTo - 3600` stays a `uint`; otherwise the value goes to the
wider type (`uint` < `int` < decimal). All arithmetic is checked: an
overflow, a `uint` below zero, a division by zero or a decimal past about
7.9e28 (e.g. `order.sellAmount / 1e20` on a huge amount) skips the rule for
that order. Comparisons have no such limit.

### Fee functions

The only values a `fee` expression may return. Arguments are expressions,
so a fee can depend on the order. A list stacks several fees. Every argument
must land on a grid of 1e-6 of the volume (two decimal places for `bps`,
four for a percent); a finer value is rejected, not rounded.

A fee, or any part of one, that reads nothing from the order is computed
once when the rules compile, so a constant `fee` costs nothing per order,
and a constant argument off the grid refuses the rule set.

- `volume(bps)` — `bps / 10000` of the volume.
- `surplus(share, max_volume_bps)` — `share` percent of the surplus over the
  limit price, at most `max_volume_bps / 10000` of the volume.
- `price_improvement(share, max_volume_bps)` — `share` percent of the
  improvement over the quote, with the same cap.
- `no_fee()` — no protocol fee.

```
fee: [volume(order.kind == "sell" ? 2 : 5), price_improvement(30, 50)]
fee: no_fee()
```

### Lists

Each entry of the lists map is a set under its name; `in` is one hash
lookup whatever the size of the list. A member that parses as a `0x`
address is an address, in any letter case; any other member is a string,
compared as written. The names `order`, `now` and `math` are reserved.
`in` is the only operation on a list.

```
when: order.sellToken in stablecoins && order.buyToken in stablecoins
when: order.appData.?appCode.orValue("") in partner_apps
```

### `now`

The time passed to `RuleSet::evaluator` as a timestamp, the same for every
order evaluated through it. It compares with `timestamp("…")` and adds or
subtracts a `duration("…")`. Duration units are `h`, `m`, `s`, `ms`, `us`
and `ns`; there is no `d`. `timestamp` of a number reads it as Unix
seconds.

```
when: now < timestamp("2026-12-01T00:00:00Z")
when: now > order.creationDate + duration("24h")
```

### `order`

The fields of the API order that a fee can depend on, under the API names
except for the app data (see `order.appData`). Each name reads the order
directly (`src/order_view`); no view is built. An order with every field
set looks like this (hashes shortened):

```json
{
  "owner": "0x40a50cf069e992aa4536211b23f286ef88752187",
  "creationDate": "2026-10-28T14:03:11.520Z",
  "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  "buyToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  "receiver": "0x0000000000000000000000000000000000000000",
  "sellAmount": 2500000000,
  "buyAmount": 621000000000000000,
  "kind": "sell",
  "partiallyFillable": false,
  "signingScheme": "eip712",
  "validTo": 1793284991,
  "validFrom": 1793281391,
  "appData": {
    "appCode": "CoW Swap",
    "metadata": {
      "flashloan": true
    }
  },
  "fastPath": false,
  "inMarket": true,
  "ethflowData": {
    "userValidTo": 1793284991
  }
}
```

The types: `owner`, `sellToken`, `buyToken` and `receiver` are addresses;
an address compares with a string literal in any letter case, and is the
lowercase hex where a string is expected (`startsWith`). `creationDate` is
a timestamp. `kind` and `signingScheme` are strings. The amounts and times
are numbers, see [Numbers](#numbers).

`inMarket` is `OrderView::in_market` as the host passes it. The engine does not
check it: the host must pass false when the order has no quote.

Execution state (`executed*`, `status`, `invalidated`, `availableBalance`,
`gasCost`), settlement fields (`signature`, `interactions`,
`settlementContract`), legacy fields (`class`, `isLiquidityOrder`), fields no
fee has depended on (`uid`, `feeAmount`, the token balances, the onchain
placement) and the quote are not here: a quote request has no quote, so a
fee that read it could not be quoted. Naming one of them does not compile.
An unset `receiver` is the zero address. Other optional fields are absent
when unset. Reach one with the optional operator `.?`: it yields an absent
value instead of failing and reaches through an optional parent, so
`order.?ethflowData.userValidTo` reads on any order, where
`order.ethflowData.userValidTo` needs a `has(order.ethflowData)` guard
first. End the chain with `.orValue(d)` for a plain value or `.hasValue()`
for a bool.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
when: timestamp(order.validTo) - now > duration("1h")
when: order.?ethflowData.hasValue()
fee:  order.inMarket ? price_improvement(50, 100) : surplus(50, 100)
```

### `order.appData`

`OrderView::app_data`: the app data a fee can depend on, which the host
copies from the protocol's parsed document, plus its `appCode`. The engine
reads no app-data text. In the order the API returns, `appData` is the hash
and `fullAppData` is the document as text; here `appData` is the parsed
document, and no hash.

The order above shows every field: the `appCode`, and `metadata.flashloan`
as `true` when the document asks for a flashloan. No other key of the
document is here, hooks, wrappers and partner fees included (see `partner_fees` under
Outcome). The host passes `AppData::default()` and no partner fees when the
order has no document or the protocol cannot parse it. `appData` and
`appData.metadata` are always present; every field under them is optional,
so reach it with `.?` or guard it with `has`.

`metadata.enableFastPath` and `metadata.validFrom` are not here: read
`order.fastPath` and `order.validFrom`, which hold the values in effect.

`appCode` is free text in the case the wallet chose, and the same wallet
does not always choose the same. Compare it through `lowerAscii()` against
a lowercase literal; a plain `==` against a mixed-case literal matches only
that spelling.

```
when: order.appData.?appCode.orValue("").lowerAscii() == "cow swap"
when: has(order.appData.metadata.flashloan)
```

## Outcome

`Evaluator::evaluate` returns an `Evaluation` with the matches and the
diagnostics, and logs nothing itself:

- `matched`: a `Match { rule, fees }` for every rule whose `when` held, in
  rule order. Empty means no rule matched, so the order pays no protocol
  fee. The caller decides which matches to act on.
- `skipped`: every rule that failed to evaluate, in rule order, each with
  the field (`when` or `fee`) and the `RuleError`. The caller decides how to
  log or count them.
- `partner_fees`: the fees `OrderView::partner_fees` asks for, in app-data
  order, also when no rule matched. The caller puts them after the fees of
  the match it picks. They are capped by `max-volume-bps` as the autopilot
  caps them; a fee past the cap stays, with a zero volume share or volume cap.
  Without `[partner-fees]` the list is empty.
