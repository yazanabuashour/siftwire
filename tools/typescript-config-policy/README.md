# Vendored TypeScript configuration policy

This directory is a dependency-free consumer snapshot of the private
TypeScript configuration policy. `SOURCE.json` records its source and content
digest. Build and verify changes in that repository, then run its
`npm run vendor -- /absolute/consumer/tools/typescript-config-policy` command
to replace this snapshot.

The consumer's active TypeScript configuration selects one of these profiles.
Follow the consumer repository's active gate instructions after updating it.
