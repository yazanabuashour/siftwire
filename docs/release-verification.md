# Verify a release

A SiftWire release publishes exactly nine assets:

- four `siftwire_<version>_<os>_<arch>` binaries
- `siftwire_<version>_skill.tar.gz`
- `siftwire_<version>_source.tar.gz`
- `siftwire_<version>_checksums.txt`
- `siftwire_<version>_sbom.spdx.json`
- `install.sh`

The installer verifies exactly one checksum entry for the selected binary,
checks that it reports the requested tag, stages replacement, then tells the
operator to register the same-tag skill. The release workflow verifies the full
asset set before publishing its draft and attests every asset. Published tags
and assets are immutable.

## Verify a release

Download all assets for one tag, then run:

```bash
shasum -a 256 -c siftwire_<version>_checksums.txt
gh attestation verify siftwire_<version>_<os>_<arch> --repo yazanabuashour/siftwire
gh attestation verify siftwire_<version>_skill.tar.gz --repo yazanabuashour/siftwire
gh attestation verify siftwire_<version>_source.tar.gz --repo yazanabuashour/siftwire
gh attestation verify install.sh --repo yazanabuashour/siftwire
```

Verify the latest pointer separately:

```bash
gh release view --repo yazanabuashour/siftwire --json tagName --jq .tagName
```

Fix a bad artifact with a new patch release; never replace a published asset.

## Smoke-test installation

Set `tag` to the release you are verifying. Use an explicit temporary install
directory so the test does not replace your installed runner:

```bash
(
  set -eu
  tag=v0.9.2
  install_dir="$(mktemp -d)"
  trap 'rm -rf "$install_dir"' EXIT
  SIFTWIRE_INSTALL_DIR="$install_dir" \
    SIFTWIRE_VERSION="$tag" \
    sh -c "$(curl -fsSL "https://github.com/yazanabuashour/siftwire/releases/download/$tag/install.sh")"

  test "$("$install_dir/siftwire" --version)" = "siftwire $tag"
  "$install_dir/siftwire" --help
)
```

For process actions and operator commands, use the [runner contract](runner-contract.md) and
[run history guide](run-history.md). Register the same-tag
`skills/siftwire/SKILL.md` before treating a production installation as complete.

## Software bill of materials

Inspect the SPDX JSON asset with audit tooling or directly:

```bash
jq '.packages | length' siftwire_<version>_sbom.spdx.json
```

The workflow generates it from the tagged source and attaches it to the same
release as the binaries, skill, installer, and source archive.
