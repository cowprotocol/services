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
typed Rust closure (see [How a rule compiles](#how-a-rule-compiles)). It
supports the subset of CEL listed under
[Names a rule can use](#names-a-rule-can-use); anything else does not
compile.

A mistake in a rule shows at one of three points:

1. When the rules compile. `RuleSet::new` refuses the whole set when a rule
   name is empty or repeats, a list takes a reserved name or an expression
   does not compile. An expression does not compile when it does not parse,
   names an unknown field, list or function, mixes types
   (`order.sellAmount == "x"`, a `when` that is not a bool), or has a
   constant part that fails, such as a fee argument off the grid
   (`volume(0.001)`), in any branch.
2. On the sample orders. `RuleSet::validate_on_sample_order` is a separate
   pass over two sample orders, so one run shows the mistakes that compiling
   cannot find. On the full sample (every optional field present) both
   expressions of every rule run, whatever `when` says, and every one that
   fails is reported. On the bare sample (no optional field present) the
   rules run as on a real order, and every rule that is skipped is reported.
   This catches a rule that reads a field an order may lack, such as
   `order.ethflowData.userValidTo`, without a `has(order.ethflowData)`
   guard: it would be skipped on every order that is not an ethflow order.
3. On an order. A rule whose expression fails on an order (a missing
   optional field, an arithmetic overflow, a computed fee argument off the
   grid) is skipped for that order and reported to the caller.

The pass over the sample orders has limits:

- A ternary, `&&` or `||` runs only what the sample order reaches.
- Every amount in the samples is 1, and every time is the time of the
  check. A fee
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
when = 'has(order.appData.appCode) && order.appData.appCode == "Trust Wallet"'
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

`RuleSet::evaluate` reads `order` from the `OrderView` the host passes. The
lists and every constant part of an expression are bound when the rules
compile.

### Operators and functions

- Literals: bools, numbers (`2`, `2u`, `0.3`, `1e24`), strings and lists of
  fees or of strings.
- `&&`, `||`, `!`, `? :`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `in`. As in
  CEL, a false operand decides `&&` and a true one `||`, also when the
  other operand fails.
- `+ - * / %` and unary `-` on numbers.
- `has(...)`; see the `order` section.
- The [fee functions](#fee-functions).

Not supported:

- The other macros (`exists`, `all`, `filter`, `map`); test a list with
  `in`.
- The optional operator `.?`; guard an optional field with `has`.
- Functions called on a value (`startsWith`, `lowerAscii`, `size`,
  `matches`).
- Conversions (`int`, `uint`, `double`, `string`), the `math` functions and
  the rest of the CEL extensions.
- `now`, `timestamp` and `duration`: a rule does not see the current time,
  and the times of an order are numbers.
- Maps and indexing.

A rule that uses one of them does not compile.

```
when: order.kind == "sell" && order.sellAmount > 1e21
fee:  surplus(order.partiallyFillable ? 40 : 50, 100)
fee:  volume(order.sellAmount > 1e21 ? 1 : 2.5)
```

### Numbers

Numbers are exact; there are no doubles.

- `uint` is 256 bits: `sellAmount`, `buyAmount`, `creationDate`, `validTo`,
  `validFrom`.
- `int` is 256 bits: `ethflowData.userValidTo` and integer literals.
- A number with a fraction is a decimal (`rust_decimal`, 28 digits), as are
  literals such as `0.3` and `1e21`. The engine reads each literal from the
  rule text, so `1e24` is exactly 10^24. A whole literal too large for a
  decimal (`1e30`) is an integer.

Values of different types compare exactly. In arithmetic both sides must
have one type:

- An integer constant takes the type of the other side when its value does
  not change, so `order.validTo - 3600` stays a `uint`.
- A decimal constant makes the operation decimal, so
  `order.sellAmount / 1e20` keeps its fraction.
- Otherwise both sides go to the wider type (`uint` < `int` < decimal).

All arithmetic is checked: an overflow, a `uint` below zero, a division by
zero or a decimal past about 7.9e28 (e.g. `order.sellAmount / 1e20` on a
huge amount) skips the rule for that order. Comparisons have no such limit.

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

Each entry of the lists map is a set under its name; `in` is one lookup
whatever the size of the list. A member that parses as a `0x`
address is an address, in any letter case; any other member is a string,
compared as written. The name `order` is reserved.
`in` is the only operation on a list.

```
when: order.sellToken in stablecoins && order.buyToken in stablecoins
when: has(order.appData.appCode) && order.appData.appCode in partner_apps
```

### `order`

The fields of the API order that a fee can depend on, under the API names
except for the app data (see `order.appData`). Each name reads the order
directly (`src/order_view`); no view is built. An order with every field
set looks like this (hashes shortened):

```json
{
  "owner": "0x40a50cf069e992aa4536211b23f286ef88752187",
  "creationDate": 1793196191,
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
an address compares with another address, or with a string literal in any
letter case. `kind` and `signingScheme` are strings. The amounts are
numbers, and so are the times (`creationDate`, `validTo`, `validFrom`,
`ethflowData.userValidTo`), in Unix seconds; see [Numbers](#numbers).

`inMarket` is `OrderView::in_market` as the host passes it. The engine does not
check it: the host must pass false when the order has no quote.

Execution state (`executed*`, `status`, `invalidated`, `availableBalance`,
`gasCost`), settlement fields (`signature`, `interactions`,
`settlementContract`), legacy fields (`class`, `isLiquidityOrder`), fields no
fee has depended on (`uid`, `feeAmount`, the token balances, the onchain
placement) and the quote are not here: a quote request has no quote, so a
fee that read it could not be quoted. Naming one of them does not compile.
An unset `receiver` is the zero address. Other optional fields are absent
when unset, and a rule that reads an absent field is skipped for that
order. Guard the read with `has`:
`has(order.validFrom) && order.validFrom > 0`. `has` of a field under an
absent object is false, so `has(order.ethflowData.userValidTo)` is safe on
any order.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
when: order.validTo - order.creationDate > 3600
when: has(order.ethflowData) && order.ethflowData.userValidTo > 0
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
so guard it with `has`.

`metadata.enableFastPath` and `metadata.validFrom` are not here: read
`order.fastPath` and `order.validFrom`, which hold the values in effect.

`appCode` is free text in the case the wallet chose, and the same wallet
does not always choose the same. `==` and `in` match one spelling only, so
put every spelling a wallet uses in a list.

```
when: has(order.appData.appCode) && order.appData.appCode == "CoW Swap"
when: has(order.appData.metadata.flashloan)
```

## Outcome

`RuleSet::evaluate` returns an `Evaluation` with the matches and the
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

## How a rule compiles

`src/compile` turns each expression into a closure, once, in
`RuleSet::new`:

1. The `cel` crate parses the text into a tree.
2. `compile::expression` walks the tree from the leaves up. Each part
   becomes a `Node`: its type (bool, number, string, address, fee, list)
   and its value, which is a constant or a closure that reads the order.
3. An operator or function checks the types of its operands, then builds
   its value from theirs. When every operand is a constant, the result is
   computed at once, so a part that reads nothing from the order costs
   nothing per order. A constant that fails is a compile error.
4. `order.<field>` becomes a call to a Rust accessor from the field table
   in `src/order_view`. The engine builds no copy of the order.
5. A list becomes a set, and `in` a lookup in it.

In `order.sellToken in stablecoins && order.sellAmount > 1e21`, the first
operand is a set lookup on the sell token. The second is a 256-bit integer
comparison: `1e21` is whole, so it is converted to a `uint` when the rule
compiles. `&&` does not run the second operand when the first is false.

To add a field, add a row to its object's table in `src/order_view`. To add
a function, add an arm to `call_node` in `src/compile/mod.rs`. Document
both here with an example line:
`tests/readme.rs` compiles every `when:` and `fee:` line of this file.
