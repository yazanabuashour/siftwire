# SiftWire

SiftWire turns feeds, GitHub releases, and sports schedules into a brief for
your agent to deliver. You choose the sources; your agent selects the optional
stories and sends the prepared message.

- **News:** Major and Highlights feeds supply stories from the last 24 hours,
  with original feed links. Required feeds track new entries instead. GitHub
  releases are always Required.
- **Sports:** Upcoming fixtures and results have separate reporting windows.
- **History:** Saved briefs preserve the selected Markdown, plain text, and
  HTML. Delivery is recorded only after your agent confirms transport acceptance.
- **Local state:** The runner and SQLite database stay on your machine.

Your agent supplies story selection, scheduling, and delivery tools. SiftWire
has no scheduler or email service. Duplicate checks and delivery history help
reduce repeats, but do not recognize every version of an event or verify article
contents. Coverage depends on what your sources publish and retain.

## Install

If you are upgrading from v0.8.0 or earlier, read the
[compatibility reset](docs/runner-v5-reset.md) first: you need a fresh database
and matching consumers. Upgrading from v0.9.x preserves your database and
configuration. v0.15.0 uses [runner v6](docs/runner-contract.md#upgrade-from-v5)
with RSS release routing. Back up your database before its additive
[schema upgrade](docs/runner-contract.md#storage-selection). Update process
consumers together when moving from v5.

Install the latest published runner:

```bash
sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/latest/download/install.sh)"
siftwire --version
```

To pin the latest documented release, v0.15.0:

```bash
SIFTWIRE_VERSION=v0.15.0 sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/download/v0.15.0/install.sh)"
siftwire --version
```

Register the matching `skills/siftwire/SKILL.md` with your agent's skill system.
For v0.15.0, use the `v0.15.0` repository tag or the release asset
`siftwire_0.15.0_skill.tar.gz`. You need both the runner and its matching skill;
no particular agent or skill directory is required.

When upgrading, update the runner, skill, and process consumers together.

The installer accepts no arguments. `SIFTWIRE_VERSION` selects the release.
`SIFTWIRE_INSTALL_DIR` selects the executable directory. Follow any printed
`PATH` instruction before verifying the runner.

## Configure your brief

Ask your agent to show your current configuration, grouped by releases, blogs,
news, forums, sports, publisher rules, and brief settings. It reads the installed runner;
no web server is needed. For changes, ask it to propose sources and reporting
choices, then approve the configuration before it writes anything. A fresh
database has no sources. The [agent skill](skills/siftwire/SKILL.md) covers
configuration and delivery.

For direct process integration, each command reads one JSON request from stdin
and returns one JSON result:

```bash
printf '%s\n' '{"action":"init"}' | siftwire config
printf '%s\n' '{"action":"inspect_config"}' | siftwire config
```

The default database is
`${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`. `XDG_DATA_HOME` must be
absolute. Use `SIFTWIRE_DATABASE_PATH` for another database, or `--db` for an
explicit dataset. Keep configuration, databases, and delivery history outside
this repository. See the [runner contract](docs/runner-contract.md) for actions,
result fields, and retry rules.

## Find a task

- [Read run and delivery history](docs/run-history.md)
- [Operate the scheduled automation](docs/automation.md)
- [Integrate the JSON runner](docs/runner-contract.md)
- [Understand current-news selection](docs/architecture/current-news.md)
- [Develop and run checks](CONTRIBUTING.md)
- [Verify release assets](docs/release-verification.md)
- [Report a vulnerability privately](SECURITY.md)

The [documentation index](docs/README.md) links to migration, architecture, and
evaluation guidance.
