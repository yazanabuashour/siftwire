# anti-slop provenance

This is a project-owned conservative port from
[dmmulroy/anti-slop](https://github.com/dmmulroy/anti-slop/tree/95a56e5d24fb3d849673c2d51eb0908b8bd2d33b),
commit `95a56e5d24fb3d849673c2d51eb0908b8bd2d33b`. The upstream MIT license and
copyright notice are preserved verbatim in [LICENSE.anti-slop](LICENSE.anti-slop)
and included in the package.

## Base and ownership

The pre-port local revision is `fa352b7`. Local history begins with an already
adapted policy in `08231ed`; it contains no pristine anti-slop snapshot or
verifiable upstream revision. The original upstream base is unknown. This is
not a three-way merge or a claim that the whole package matches upstream.

Canonical upstream inputs are `src/rules/`, `src/shared/`, and
`src/effect/rules/no-service-constructor-imports.ts`, plus their tests, README,
and `skills/install-anti-slop/references/update.md`. No installer or skill assets
are part of this package. Future updates must compare these pinned inputs with
local behavior, preserve the differences below, and record unresolved conflicts.

## Classification

- Added and enabled the generic `no-array-filter-map` and
  `no-reduce-accumulator-copy` rules with native `oxc/no-accumulating-spread`.
- Added `no-service-constructor-imports` through the named `effectConfig` export,
  under `project-effect`. Only direct Effect consumers opt in. No automatic
  dependency detection or Effect dependency was added.
- Ported existence probes and optional predicate checks for `no-runtime-typeof`;
  static borrowed member names for `no-shape-in-symbol-names`; exact predicate
  subjects for `no-unknown-parameters`; and non-empty configurable safety markers,
  including comments above exports. Both requested bans remain enabled.
- Ported scoped, forward, and transparent generic alias resolution for object
  parameters, unknown returns and aliases, dictionaries, and widening targets.
  Finite-key `Record` targets remain valid. Dictionary constraints are allowed;
  unsafe dictionary defaults and concrete contracts remain rejected. Widening
  analysis now checks known arguments to local unknown-input predicates.
- Already equivalent: chained assertions, conditional empty spreads, module
  mocking, Reflect get/apply, and immutable widen-then-assert flows. Their local
  diagnostics and detection remain intact.
- Preserved all personal rules, native settings, consumer ownership, measured
  tripwires, and the narrow local AST adapter override.

## Intentional implementation differences

The plugin namespace remains `project`, registered with `definePlugin`. The
package exports compiled JavaScript and declarations instead of upstream's
source entry point. Oxlint and `@oxlint/plugins` remain pinned together at
`1.78.0`, matching upstream.

Unknown parameters retain local alias detection and now share upstream's lexical
resolver with unknown aliases. Upstream's parameter rule only checks direct
unknown syntax. Dictionary and widen-then-assert helpers remain split to respect
local measured line tripwires. Shared parameter and child-node helpers replace
repeated traversal logic; the AST adapter uses visitor keys without upstream's
chained casts. Options use Oxlint's validated schema contract and per-file
`create` rather than caching marker regular expressions across files. Block-comment
star prefixes do not count as a justification. This closes an upstream gap in
the advertised non-empty-comment contract.

Known predicate calls intentionally follow upstream's direct unknown-input
syntax check and same-file signatures. Dictionary interface analysis still only
collects top-level interfaces. Neither implementation claims full TypeScript
inference. No policy conflicts were deferred; the original-base limitation
remains for future updates.

## Verification

`npm run check` covers formatting, typechecking, the build, registered-plugin and
rule-contract tests, lint, and package contents. Tests cover the new generic and
optional rules, changed contracts, and preserved personal behavior. No consumer
code or instruction files belong to this port.
