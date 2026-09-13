# Frozen email HTML

These synthetic HTML files were rendered by SiftWire at
`b70191e2a4d67483e5720375b83cb3e81d50f436`, before extracting its markup and styles
into the standalone `email-ui` crate. The renderer's production source was checked
byte-for-byte against that starting version before capture. The golden files
were not regenerated from the shared renderer.

Run the mapper's exact-byte assertions from the repository root:

```bash
mise exec -- cargo test --locked -p siftwire --bin siftwire runner::email::tests
```

| Fixture | Distinct coverage | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `rich-evening.html` | Existing rich test: releases, news, both sports groups, health, empty-link spans, one image, Chicago date rollover and CDT | 6471 | `f8798c01f5479302846a64842842ec3249ba804998f321e8cb00949a5742bfe5` |
| `morning-escaped-images.html` | Morning/JST rollover, interleaved input regrouping, plural counts, all HTML escape characters, section/outlet fallbacks, three supplied images with only the first two displayed | 6741 | `fb0af90ac273849a393aa31fe1eeddc28f3ddb71ffbb3a6ce050a66951704479` |
| `empty-noon.html` | Noon selects Evening; zero counts; no content sections or health notice | 1990 | `b7a4d89f9ca0a49090220b80089cad8f5120c5805c51ca9838508a996c91bced` |
| `unknown-sports-morning.html` | Before noon selects Morning; unknown sports status counts but leaves an empty Sports wrapper | 2202 | `9c30e08b906e5af862b9de00242afc9d17196b42c267efd94b9787811a3f24bc` |

The shared-renderer outputs matched all four frozen files exactly. The HTML is
browser-ready; external image hosts are synthetic and may not load. This is a
byte-parity receipt, not an email-client compatibility or live-delivery claim.
The empty fixture exercises the HTML renderer directly; delivery preparation
still returns an empty HTML body when the complete message is `NO_REPLY`.

The formatter excludes these HTML files because whitespace is part of the
asserted output. Intentional visual changes need a new reviewed baseline; do not
update the files merely to make a failing migration test pass.
