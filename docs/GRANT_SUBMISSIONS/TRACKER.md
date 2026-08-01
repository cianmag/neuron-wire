# Grant Submission Tracker

> Live status of every grant application. **Update after every action.**
> Verified 2026-08-01 · next re-verify: 2026-08-15 (programs change — NLnet is the live example).

## Rules of engagement
1. Submit only to programs verified **active** (✓) at submission time — re-verify the week you submit.
2. Send the **two-page technical summary** (`docs/TECHNICAL_SUMMARY.md`) first; point to the full
   evidence report (`docs/EVIDENCE_REPORT.md`) and the **GitHub release** (not moving master):
   https://github.com/cianmag/neuron-wire/releases/tag/v0.3.1
3. One evidence core, tailored framing per funder: same 342 tests / 20-node pilot / $20k budget.
4. Log every submission, reply, deadline, and follow-up below. No silent waiting.
5. **Never submit a program that is paused, restricted, or legally inaccessible** — the fields
   below exist to catch this before an afternoon is wasted.

## Eligibility fields (check EVERY program against these before any submission)
| Field | Meaning |
|-------|---------|
| **Applicant age requirement** | Minimum age at award date (e.g. Rust Foundation: 18). Applicant is 16, turns 17 Oct 2026. |
| **Eligible legal recipient** | Who can legally receive funds: self / guardian / company / fiscal sponsor / institution. |
| **Payout method** | Fiat, crypto, credits; asset defined; exchange-rate date; vesting; custody. |
| **Tax jurisdiction** | Nepal / India / US receipt — transfer and tax implications for the recipient. |
| **Ecosystem dependency** | Is the program for THIS ecosystem (e.g. Filecoin)? "Both decentralized" is NOT sufficient. |
| **Submission status** | OPEN / BLOCKED / PAUSED / SUBMITTED / REJECTED / AWARDED — with reason. |

## Priority order (2026-08-01, per review decision)

| # | Program | Type | Submission status | Draft | Next action | Deadline |
|---|---------|------|-------------------|-------|-------------|----------|
| 1 | **DigitalOcean Startups/Hatch** | Credits | ⚠️ **BLOCKED: no company website** — zylvon.com resolves (Cloudflare DNS) but returns HTTP 000. DO requires: company website + matching corporate email, registered team account with business email, ≤$10M raised, <24-month company, AI-native prioritized. Verify which route actually gives the claimed $250 (some credit levels need accelerator/investor partnerships) | — | Stand up minimal zylvon.com site; verify the $250 route; then apply | Rolling |
| 2 | **Rust Foundation Community Grants** | Cash | ⚠️ **BLOCKED: applicant under 18** (official policy: over 18 by award date + able to receive US transfers). 2026 round not confirmed open. | `RUST_FOUNDATION.md` ✅ (full draft, title "Hardening and Reproducing a Synchronous Rust Runtime for Decentralized Learning") | Ask grants@rustfoundation.org: (a) fiscal-sponsor/institutional pathway, (b) age waiver w/ guardian consent, (c) Nepal receipt; verify 2026 round + deadline | Verify before submitting |
| 3 | **NLnet — Open Internet Stack** (Restack / CodeSupply / ELFA) | Cash | ⚠️ **PAUSED until post-summer 2026** | `NLNET_OIS.md` ✅ | Re-verify nlnet.nl/propose weekly; submit within 2 weeks of reopen | Post-summer reopen |
| 4 | **Filecoin Foundation Next Step** | Cash (crypto) | ⚠️ **CONCEPT ONLY — go/no-go pending** (no real Filecoin integration yet) | `FILECOIN_CONCEPT.md` ✅ (one-page concept: verifiable storage + reproduction via IPFS/Filecoin) | Go/no-go per criteria in concept: program confirms scope qualifies AND integration worth building independently; then payout-terms check | Rolling |
| 5 | **Protocol Labs Research** | Cash | ⚠️ **PAUSED indefinitely** (README 2026-06-12) | `PROTOCOL_LABS.md` (template) | Do NOT submit; re-use draft as template | n/a |
| 6 | Microsoft Founders Hub | Credits | Active — verify | — | Entry tier ~$1–5k Azure+OpenAI; India/Nepal ok | Rolling |
| 7 | AWS Activate | Credits | Active — verify | — | Needs business entity; check Nepal/India | Rolling |
| 8 | Ethereum Foundation ESP | Cash (crypto) | Active — verify | — | Verify payout/tax stance | Rolling + RFPs |

## Eligibility snapshot (all programs, 2026-08-01)
| Program | Age req | Eligible recipient | Payout | Tax jurisdiction | Ecosystem dep | Status |
|---------|---------|--------------------|--------|------------------|---------------|--------|
| DigitalOcean Startups/Hatch | 18+ (ToS) | Company | Credits (no cash) | n/a (credits) | None | BLOCKED: no website |
| Rust Foundation | **18 by award** | Self / fiscal sponsor / institution? | Fiat (US transfers) | Nepal receipt? | Rust (genuine) | BLOCKED: age |
| NLnet OIS | 18+ w/ guardian consent for negotiations (minors OK) | Self w/ guardian | Fiat (EU) | EU/Nepal | Open-source (genuine) | PAUSED: reopen post-summer |
| Filecoin Next Step | 18+ typical | Self / company | Crypto (FIL) — verify terms | Verify | **Filecoin (NOT genuine yet)** | CONCEPT ONLY |
| Protocol Labs | 18+ | Self | Fiat/crypto | Verify | Decentralized-computing (genuine) | PAUSED |
| MS Founders Hub | 18+ | Company | Credits | n/a | None | VERIFY |
| AWS Activate | 18+ | Company | Credits | n/a | None | VERIFY |
| Ethereum ESP | 18+ | Self | Crypto | Verify | Ethereum (genuine) | VERIFY |

## Submission log

| Date | Program | Action | Result / Reply | Follow-up |
|------|---------|--------|----------------|-----------|
| 2026-08-01 | (all) | Statuses re-verified; Protocol Labs PAUSED; NLnet Taler/Fediversity-only until post-summer; Rust Foundation age policy confirmed from official page; DO website check failed (HTTP 000) | — | Re-verify 2026-08-15; email Rust Foundation with the 3 questions; stand up zylvon.com |

## Milestones that unlock the next tier
- First cash grant → funds the 20-node three-region pilot → pilot data strengthens every later application.
- First credits (DO/MS) → free cloud → real WAN experiment → evidence for NLnet/Rust Foundation.
- Cold-email professors (distributed systems / P2P / ML labs) with the technical summary + release →
  mentor/institutional support → org-gated programs (ISOC, Open Technology Fund).

## One-command evidence reference (for every application)
```
Version:        v0.3.1
Evidence commit: 265e2169949c180d7ad3a0ce0dcf73c4b75687b7
CI run:         30697662079   (success)
Evidence run:   30697662078   (success)
Test count:     342 passing on Linux CI
Release:        https://github.com/cianmag/neuron-wire/releases/tag/v0.3.1
```
