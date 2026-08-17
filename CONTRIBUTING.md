# Contributing

Install the pinned tools and run the repository gate when changing Siftwire:

```bash
mise install
mise exec -- ./scripts/ci.sh
```

Do not commit personal source inventories, `.openclaw` content, workspace
backups, delivery logs, run history, or local SQLite databases.

When a change affects the public runner, skill, release, or security contract,
update the matching docs and release notes. Before tagging a release, run:

```bash
mise exec -- ./scripts/validate-release-docs.sh <tag>
```

Use GitHub issues and pull requests for tracked project work.
