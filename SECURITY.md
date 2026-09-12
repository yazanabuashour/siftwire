# Security policy

## Supported versions

SiftWire ships a local runner and a single-file agent skill. The current default
branch and the latest `0.y.z` tag are supported. Older pre-`1.0` tags may not
receive fixes or backports.

## Reporting a vulnerability

Do not report vulnerabilities in public issues, pull requests, or discussions. Do not report private brief data, source inventories, outlet policy details, delivery history, local database contents, or workspace paths in public.

Use GitHub private vulnerability reporting from the repository **Security** tab.
Include the following details:

- a clear description of the issue
- affected files or workflow surfaces
- reproduction steps or proof-of-concept details
- expected impact and known mitigations

If GitHub private reporting is unavailable, contact the repository owner through an existing private channel and share only enough detail to arrange private handoff. Do not disclose the vulnerability publicly while that handoff is being arranged.

## Response expectations

These targets are not guarantees.

| Severity | Initial acknowledgment | Status update target | Patch or mitigation target |
| --- | --- | --- | --- |
| Critical | within 2 business days | within 5 calendar days | within 14 calendar days |
| High | within 3 business days | within 7 calendar days | within 30 calendar days |
| Medium | within 5 business days | within 14 calendar days | next planned release or documented mitigation |
| Low | within 5 business days | as needed | next routine release if accepted |

## Severity handling

Maintainers assign severity by impact on users and maintainers:

- Critical: repository compromise, credential exposure, arbitrary code execution in trusted automation, release-integrity failure, or broad disclosure of private brief configuration or delivery history.
- High: meaningful integrity, privacy, or privilege risk without a full repository compromise.
- Medium: exploitable weakness with limited blast radius or clear prerequisites.
- Low: hard-to-exploit issue, defense-in-depth gap, or low-impact misconfiguration.

## Ongoing security operations

[Security operations](docs/security-operations.md) covers dependency reviews,
advisory rehearsals, threat-model updates, and deeper testing. This policy defines
private reporting and response expectations.

## Patch and advisory process

- Fixes land privately first when needed to avoid widening exposure.
- Public release notes should avoid exploit-enabling detail until a fix or mitigation is available.
- If the repository later adopts GitHub Security Advisories, maintainers should publish advisories for material fixes.

## Emergency releases and hotfixes

If a vulnerability affects the latest supported code line, maintainers may
publish an emergency patch tag and GitHub Release.

Emergency fixes publish updated binary, skill, and source releases with checksums, SBOMs, and GitHub attestations. SiftWire does not publish a hosted service deployment or remote HTTP API contract.
