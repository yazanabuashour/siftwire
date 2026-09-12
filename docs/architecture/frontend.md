# Frontend ownership and shared policies

SiftWire uses React, Vite, Tailwind, TanStack Query, and Zod with shared compiler
and lint policies. The console has one client and a Rust server that calls the
installed runner, so separate schema, client, and view packages would add
boundaries without another consumer.

## Module responsibilities

The frontend separates transport, decoding, cached data, and views.

- `apps/web/src/http-client.ts` owns HTTP requests, response parsing, timeouts,
  and cancellation cleanup.
- `apps/web/src/api-contracts.ts` owns Zod response schemas and runner
  compatibility checks. Views receive successful responses only after validation
  against these schemas.
- `apps/web/src/api-client.ts` supplies resource requests and their response
  schemas. Configuration mutations decode their own partial result shapes.
- `apps/web/src/components/config-query.ts` and `run-queries.ts` own domain query
  keys and cache behavior. Configuration updates preserve fields outside the
  response's ownership and cancel stale reads before merging results.
- `apps/web/src/navigation.tsx` owns URL navigation. `App.tsx` selects pages.
  Editors keep unsaved changes separate from saved configuration.

The Rust console never opens SQLite or imports runner internals. Its process
client owns argument construction, JSON framing, and adaptation of signed
64-bit priorities to decimal strings. JavaScript numbers cannot represent the
full signed 64-bit range exactly.

## Failures and cancellation

Response parsing preserves JSON errors and abort causes. An HTTP failure with
an unreadable body retains both the status and the original cause. The client
does not invent wire error codes that the console does not supply.

Automatic query and mutation retries are disabled. Reads can recover through
explicit refetch or the existing focus-refetch behavior. Writes are not replayed
because a browser request failed.

The request owner forwards cancellation and removes its listeners and timer on
every exit. A browser abort stops the browser's wait. It does not prove that a
runner write did not commit.

## Policy distribution

The compiler configuration extends the checked-in snapshot under
`tools/typescript-config-policy/`. Oxlint imports
`@yazanabuashour/oxlint-config` from the local package under
`tools/typescript-lint-policy/`. Neither path requires credentials for another
repository during installation.

The policy projects own their source and export commands:

- [TypeScript configuration policy](https://github.com/yazanabuashour/typescript-config-policy)
- [TypeScript lint policy](https://github.com/yazanabuashour/typescript-lint-policy)

Each snapshot records its source in `SOURCE.json`. The compiler snapshot records
a content digest. The lint snapshot includes compiled code and the required
license notices. The source release archive includes these snapshots, so a
console build does not depend on mutable upstream branches.

Shared rules do not replace runtime checks. SiftWire retains Zod decoding and
`noPropertyAccessFromIndexSignature`. Runtime, JSX, build, and test settings stay
local to each consumer.
`tsconfig.tooling.json` checks root tooling files.

## Changes that need a caller

Typed routing is a candidate for a future navigation change, not a prerequisite
for a flat console. Any replacement must preserve deep links, browser history,
and pending-write behavior. Native dialogs already provide browser-owned
modality; replacing them with a component library alone would not improve that
contract.

SiftWire keeps its installed-runner boundary and its existing visual design.
New packages or frameworks need a concrete task that the current modules cannot
handle cleanly.
