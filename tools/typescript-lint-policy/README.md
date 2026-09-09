# TypeScript lint policy

`@yazanabuashour/oxlint-config` is the project-owned Oxlint policy for Yazan's
TypeScript repositories. It is private and is not published to a package
registry.

The default export combines native correctness, suspicious, performance,
React Hooks, TypeScript safety, and accessibility rules with the generic
[anti-slop rules](https://github.com/dmmulroy/anti-slop/tree/95a56e5d24fb3d849673c2d51eb0908b8bd2d33b#generic-rules)
and personal Node.js import, host-runtime, Schema compilation, and Effect test
ownership rules. [UPSTREAM.md](UPSTREAM.md) records the port and local differences.

Use an exact, repository-owned snapshot so clean installs do not need credentials
for a different private repository:

```ts
import policy from "@yazanabuashour/oxlint-config"
import { defineConfig } from "oxlint"

export default defineConfig({
  extends: [policy],
})
```

## Optional Effect architecture rule

Only repositories that depend directly on Effect should add `effectConfig`:

```ts
import policy, { effectConfig } from "@yazanabuashour/oxlint-config"
import { defineConfig } from "oxlint"

export default defineConfig({
  extends: [policy, effectConfig],
})
```

This enables `project-effect/no-service-constructor-imports`. It rejects named
`make<CapabilityName>` imports from relative project modules outside `*.test.*`
and `*.spec.*` files. Import the owning Layer and yield the contextual service
instead. Package imports, path aliases, default imports, and static constructors
such as `WorkspaceName.make` are outside its scope. No dependency discovery runs
at configuration time. The existing personal Effect rules remain in the default.

## Contracts and local configuration

- Runtime `typeof` narrowing remains banned. Comparisons with the string
  `"undefined"` are allowed as existence probes. Set
  `"project/no-runtime-typeof": ["error", { allowInTypeGuards: true }]` to permit
  checks directly inside predicate and assertion functions. The default is false.
- Conditional empty-object spreads remain banned without an autofix. Omitting
  a property is not equivalent to assigning `undefined`.
- `project/require-safety-comment-for-type-assertion` requires a non-empty
  justification. Its optional `markers` array replaces the default `["SAFETY"]`.
  Comments above exported declarations count.
- Adjacent eager array filter/map passes and reducer accumulator copies are
  rejected. Native `oxc/no-accumulating-spread` covers spreads. Review callback
  order, indexes, sparse arrays, ownership, and runtime support before switching
  APIs; these rules do not autofix.
- Analysis uses syntax and lexical scope, not the TypeScript type checker.
  Same-file aliases resolve where supported; imported types and cross-file call
  signatures do not. Unknown array receivers and lazy iterator pipelines are
  outside the array rule.
- `max-lines` remains 300 effective lines and `max-lines-per-function` remains
  100. [receipts/size-limits.json](receipts/size-limits.json) records the measurements.
  Consumers own generated-file exemptions, host-runtime adapter allowlists, and
  restricted-import targets. The portable default supplies none of these.

Run `npm run check` before committing. Build changes regenerate tracked `dist/`.
The package includes the upstream MIT notice in `LICENSE.anti-slop`.
