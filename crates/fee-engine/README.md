# fee-engine

Rule-based protocol fees. A `RuleSet` holds named lists and an ordered list
of rules. For every order every rule runs; the engine reports each rule whose
`when` is true, in rule order, with its fee. The caller picks which matches
to act on, e.g. the first. The partner fees of the order's app data are not
rules: the engine returns them apart, capped, to follow the picked fees.

## Rules

A rule is a `RuleDefinition`: a name and two [CEL](https://cel.dev)
expressions. `when` must return a bool. `fee` must return one fee or a list
of fees. `RuleSet::new` refuses the whole set when a rule name is empty or
repeats, a list takes a reserved name or an expression does not compile. A
rule whose expression fails at run time is skipped and reported to the
caller.

`RuleSet::validate_on_sample_order` is a separate pass over two sample
orders, so one run shows all the mistakes. On the full sample (every
optional field present) both expressions of every rule run, whatever `when`
says, and every `when` that does not return a bool and every `fee` that does
not return valid fees is reported. This catches a misspelt name or a bad fee
argument such as `volume(0.001)` at load time instead of on the first order.
On the bare sample (no optional field present) the rules run as on a real
order, and every rule that is skipped is reported. This catches a rule that
reads a field an order may lack, such as `order.ethflowData.userValidTo`,
directly instead of through the optional operator
(`order.?ethflowData.userValidTo`), which would be skipped on every order
that is not an ethflow order.

The pass has limits:

- A ternary, `&&` or `||` runs only what the sample order reaches. The
  sample is out of market, so the bad argument in `order.inMarket ?
  volume(0.001) : volume(1)` is not found; a static check of every branch is
  planned.
- Every amount in the samples is 1, except `feeAmount`, which is 0
  as on every order the API accepts, and every timestamp is `now`. A fee
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

## Names bound once

`RuleSet::evaluator` binds the names in this section once. Each order is
then evaluated in a child scope on top of them, which adds the names of the
next section.

### The CEL standard library

The operators (`==`, `&&`, `||`, `in`, `? :`, arithmetic), the standard
functions (`size`, `startsWith`, `endsWith`, `contains`, `matches`,
`timestamp`, `duration`, `string`, `int`) and the macros (`has`, `exists`,
`all`, `filter`, `map`). Optional values are on: `a.?b` reaches a field an
order may lack without failing; see the `order` section.

```
when: order.kind == "sell" && order.sellAmount > 1e21
fee:  surplus(order.partiallyFillable ? 40 : 50, 100)
```

### Fee functions

The only values a `fee` expression may return. Arguments are CEL expressions,
so a fee can depend on the order. A list stacks several fees. Every argument
must land on a grid of 1e-6 of the volume (two decimal places for `bps`,
four for a percent); a finer value is rejected, not rounded.

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

### The CEL math extension

`math.greatest`, `math.least`, `math.ceil`, `math.floor`, `math.round`,
`math.abs`, `math.sqrt` and the rest of the
[math extension](https://github.com/google/cel-spec/blob/master/doc/extensions/math.md).
`greatest` and `least` bound a computed fee inside the rule.

```
fee: volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e20)))
```

### The CEL strings extension

`lowerAscii`, `upperAscii`, `split`, `join`, `substring`, `trim`, `replace`,
`format` and the rest of the
[strings extension](https://github.com/google/cel-spec/blob/master/doc/extensions/strings.md).
`lowerAscii` makes a pasted checksummed address compare equal to the
lowercase view, and makes an `appCode` comparison case-insensitive: `==` is
not, and the view keeps the code as the wallet wrote it.

```
when: order.sellToken == "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".lowerAscii()
when: order.appData.?appCode.orValue("").lowerAscii() == "trust wallet"
```

### Lists

Each entry of the lists map is a CEL list of strings under its name. `in`
tests membership and the list macros work on it. Hex members are lowercased
when the set is built, as are the hex strings in `order`, so address case
does not matter. The names `order`, `now` and `math` are reserved.

```
when: order.sellToken in stablecoins && order.buyToken in stablecoins
when: jit_makers.exists(m, m == order.owner || m == order.receiver)
```

### `now`

The time passed to `RuleSet::evaluator` as a CEL timestamp, the same for
every order evaluated through it.
It compares with `timestamp("…")` and adds or subtracts a `duration("…")`.
Duration units are `h`, `m`, `s`, `ms`, `us` and `ns`; there is no `d`.

```
when: now < timestamp("2026-12-01T00:00:00Z")
when: now > timestamp(order.creationDate) + duration("24h")
```

## Names bound per order

`Evaluator::evaluate` binds these in a child scope and drops them after the
order.

### `order`

The fields of the API order that a fee can depend on, under the API names
except for the app data (see `order.appData`), built field by field in
`order_view/mod.rs`. Amounts are doubles, whole numbers
(`validTo`, `validFrom`, `ethflowData.userValidTo`) are `uint`s and hex is
lowercase. An order with every field set looks like this (hashes shortened):

```json
{
  "uid": "0x5f2a…d1e6",
  "owner": "0x40a50cf069e992aa4536211b23f286ef88752187",
  "creationDate": "2026-10-28T14:03:11.520Z",
  "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  "buyToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  "receiver": "0x0000000000000000000000000000000000000000",
  "sellAmount": 2.5e9,
  "buyAmount": 6.21e17,
  "feeAmount": 0.0,
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "validTo": 1793284991,
  "validFrom": 1793281391,
  "appData": {
    "appCode": "CoW Swap",
    "metadata": {
      "signer": "0x40a50cf069e992aa4536211b23f286ef88752187",
      "replacedOrder": { "uid": "0x9b4c…e1f0" },
      "flashloan": {
        "liquidityProvider": "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2",
        "protocolAdapter": "0x7d9c4dee56933151bc5c909cfe09def0d315cb4a",
        "receiver": "0x7d9c4dee56933151bc5c909cfe09def0d315cb4a",
        "token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "amount": 2.5e9
      }
    }
  },
  "fastPath": false,
  "inMarket": true,
  "ethflowData": {
    "userValidTo": 1793284991
  },
  "onchainOrderData": {
    "sender": "0x40a50cf069e992aa4536211b23f286ef88752187"
  },
  "onchainUser": "0x40a50cf069e992aa4536211b23f286ef88752187"
}
```

`inMarket` is `OrderView::in_market` as the host passes it. The engine does not
check it: the host must pass false when the order has no quote.

Execution state (`executed*`, `status`, `invalidated`, `availableBalance`,
`gasCost`), settlement fields (`signature`, `interactions`,
`settlementContract`), legacy fields (`class`, `isLiquidityOrder`) and the
quote are not in the view: a quote request has no quote, so a fee that read it
could not be quoted. An unset `receiver` is the zero address. Other optional
fields are absent when unset, not `null`. Reach one with the optional
operator `.?`: it yields an absent value instead of failing and
reaches through an optional parent, so `order.?ethflowData.userValidTo`
reads on any order, where `order.ethflowData.userValidTo` needs a
`has(order.ethflowData)` guard first. End the chain with `.orValue(d)` for a
plain value, `.hasValue()` for a bool, or `.optMap(x, …)` to compute over it.

A `uint` compares with any number, but arithmetic needs `uint` operands,
so write `order.validTo - 3600u` or `int(order.validTo) - 3600`, not
`order.validTo - 3600`. `timestamp(int(order.validTo))` makes a timestamp
of it.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
when: timestamp(int(order.validTo)) - now > duration("1h")
when: order.?ethflowData.hasValue()
fee:  order.inMarket ? price_improvement(50, 100) : surplus(50, 100)
```

A hex literal compares equal to the view only in lowercase: write it so, or
call `lowerAscii()` on it. Addresses belong in lists, where case does not
matter.

### `order.appData`

`OrderView::app_data`: the app data a fee can depend on, which the host
copies from the protocol's parsed document, plus its `appCode`. The engine
reads no app-data text. In the order the API returns, `appData` is the hash
and `fullAppData` is the document as text; the view has the parsed document
as `appData`, and no hash.

The order above shows every field. No other key of the document is here,
hooks, wrappers and partner fees included (see `partner_fees` under
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
