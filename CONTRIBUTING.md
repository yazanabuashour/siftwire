# Contributing

Install the pinned tools and run the repository gate when changing SiftWire:

```bash
mise install
mise exec -- ./scripts/ci.sh
```

The frontend uses checked-in compiler and lint policy snapshots. Follow
[frontend ownership and shared policies](docs/architecture/frontend.md) when
changing requests, schemas, cached data, or views. Update snapshots with their
producer's export command rather than editing generated policy code locally.

Do not commit personal source inventories, `.openclaw` content, workspace
backups, delivery logs, run history, or local SQLite databases.

When a change affects the public runner, skill, release, or security contract,
update the matching docs and release notes. Before tagging a release, run:

```bash
mise exec -- ./scripts/validate-release-docs.sh <tag>
```

Use GitHub issues and pull requests for tracked project work.
