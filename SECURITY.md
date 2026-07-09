# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| v0.3.0  | ✅ (current release) |
| < v0.3  | ❌ (pre-release prototypes) |

## Reporting a Vulnerability

This is a **research prototype**, not production infrastructure. Security is explicitly documented as future work — see [FOUNDATIONAL_QNA.md §8](FOUNDATIONAL_QNA.md#8-security) for the current limitations.

That said, if you discover a vulnerability that affects the safety of anyone deploying or experimenting with this software:

1. **Do not** open a public GitHub issue.
2. Send a description to the maintainer at **team@zylvon.com** (or open a [GitHub Security Advisory](https://github.com/cianmag/neuron-wire/security/advisories)).
3. Include the word "neuron-wire" in the subject line.
4. Provide detailed steps to reproduce the issue.

You should receive a response within 72 hours. If you don't, follow up.

## What to Expect

Because this is a research prototype:

- **Critical vulnerabilities** (remote code execution, data loss) will be prioritized and patched within 2 weeks.
- **Moderate issues** (partial information disclosure, denial of service) will be addressed on the regular development timeline.
- **Low-severity issues** (theoretical attacks, missing hardening) will be documented as known limitations and scheduled per the roadmap.

## Current Security Posture

As documented in `FOUNDATIONAL_QNA.md` §8, the current prototype has:

- ❌ No authentication (anyone can generate a NodeId and join)
- ❌ No encryption (wire format is plain FlatBuffer)
- ❌ No replay protection
- ❌ No Sybil resistance
- ❌ No rate limiting

These are known limitations of a research prototype. A production deployment should not be attempted without addressing all of the above. The goal of this project is to generate research evidence about decentralized learning, not to provide production-ready infrastructure.

## Preferred Languages

English is preferred for security reports.
