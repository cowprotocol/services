# fee-engine

Rule-based protocol fees. A `RuleSet` holds named lists and an ordered list
of rules. For every order the rules run top to bottom. The first `when` that
is true picks the fee.

## Rules

A rule is a `RuleDefinition`: a name and two [CEL](https://cel.dev)
expressions. `when` must return a bool. `fee` must return one fee or a list
of fees. `RuleSet::new` refuses the whole set when a rule name repeats, a
list takes a reserved name or an expression does not compile. A rule whose
expression fails at run time is skipped and logged at debug level.

```
name: Stablecoin swaps
when: order.sellToken in stablecoins && order.buyToken in stablecoins
fee:  volume(10)

name: Default
when: true
fee:  surplus(50, 1)
```

## Names bound once

`RuleSet::evaluator` binds the names in this section once. Each order is
then evaluated in a child scope on top of them, which adds the names of the
next section.

### The CEL standard library

The operators (`==`, `&&`, `||`, `in`, `? :`, arithmetic), the standard
functions (`size`, `startsWith`, `endsWith`, `contains`, `matches`,
`timestamp`, `duration`, `string`, `int`) and the macros (`has`, `exists`,
`all`, `filter`, `map`). Optional values are off, so `a.?b` does not compile.

```
when: order.kind == "sell" && order.sellAmount > 1e21
fee:  surplus(order.partiallyFillable ? 40 : 50, 1)
```

### Fee functions

The only values a `fee` expression may return. Arguments are CEL expressions,
so a fee can depend on the order. A list stacks several fees. Factors are
rounded to the 1e-6 grid `FeeFactor` compares on.

- `volume(bps)` — `bps / 10000` of the volume.
- `surplus(share, max_volume)` — `share` percent of the surplus over the limit
  price, at most `max_volume` percent of the volume.
- `price_improvement(share, max_volume)` — `share` percent of the improvement
  over the quote, with the same cap.
- `no_fee()` — no protocol fee.

```
fee: [volume(order.kind == "sell" ? 2 : 5), price_improvement(30, 0.5)]
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
lowercase view.

```
when: order.sellToken == "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".lowerAscii()
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

The order as the API returns it, with two changes: decimal strings (amounts
and prices, including the quote's) are doubles, and hex strings are
lowercase. `fullAppData` stays text. An unset `receiver` is the zero
address, not `null`.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
```

A hex literal compares equal to the view only in lowercase: write it so, or
call `lowerAscii()` on it. Addresses belong in lists, where case does not
matter.

## Outcome

`Evaluator::evaluate` returns:

- `Some(Match { rule, fees })`: the first rule whose `when` held, and its
  fees.
- `None`: no rule matched, so the order pays no protocol fee.
