# Documentation

## Set up and use SiftWire

- [Install and configure](../README.md#install): runner, matching agent skill,
  and first configuration
- [Agent skill](../skills/siftwire/SKILL.md): approved configuration writes,
  story selection, and prepared delivery
- [Console](console.md): local brief reader, source editor, and Activity view
- [Run and delivery history](run-history.md): archive commands, search,
  pagination, and saved evidence

## Upgrade an installation

- [Compatibility reset](runner-v5-reset.md): this checkout requires matching v5
  consumers and a fresh database

Historical release guides:

- [Upgrade to runner v4](runner-v4-migration.md): v0.8.0 current-news behavior,
  nullable fetch counts, and pending delivery
- [Apply the v3 migration](runner-v3-migration.md): retired writes and source
  options, also required when upgrading a v2 installation to v4

## Look up contracts

- [Runner contract](runner-contract.md): JSON framing, actions, result fields,
  storage selection, and retry rules
- [Agent production evaluation](evals/agent-production.md): scenarios, isolation,
  model selection, and release criteria
- [Evaluation reports](agent-eval-results/README.md): committed reduced evidence

## Understand design decisions

- [Current-news selection](architecture/current-news.md): publication window,
  original links, and duplicate checks
- [Frontend ownership and shared policies](architecture/frontend.md): module
  responsibilities, failure handling, and policy snapshots
- [Database-backed configuration](architecture/db-backed-configuration-adr.md):
  external configuration and state
- [Sports schedules](architecture/schedule-source-adr.md): recurring fixtures
  and results
- [AgentOps surface policy](architecture/agentops-surface-policy.md): supported
  interfaces, source intake, and approval boundaries

## Maintain and release

- [Contributing](../CONTRIBUTING.md): development setup and checks
- [Maintainer notes](maintainers.md): repository administration and releases
- [Release verification](release-verification.md): assets and attestations
- [Security operations](security-operations.md): recurring security work
