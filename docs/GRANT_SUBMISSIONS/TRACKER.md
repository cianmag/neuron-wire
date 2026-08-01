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

## Priority order (2026-08-01)

| # | Program | Type | Status | Draft | Next action | Deadline |
|---|---------|------|--------|-------|-------------|----------|
| 1 | **Rust Foundation Community Grants** | Cash | ⚠️ verify (site Cloudflare-blocked) | — | Verify program page; draft from PL template | Rolling — re-verify |
| 2 | **Filecoin Foundation dev grants** | Cash (crypto) | Active | — | Draft from PL template; verify payout form/tax | Rolling |
| 3 | **NLnet — Open Internet Stack** (Restack / CodeSupply / ELFA) | Cash | ⚠️ **PAUSED until post-summer 2026** | `NLNET_OIS.md` ✅ | Re-verify nlnet.nl/propose weekly; submit within 2 weeks of reopen | Post-summer reopen |
| 4 | **Protocol Labs Research** | Cash | ⚠️ **PAUSED indefinitely** (README 2026-06-12) | `PROTOCOL_LABS.md` (template) | Do NOT submit; re-use draft as template | n/a |
| 5 | **DigitalOcean Hatch** | Credits | Active | — | Apply (lowest-effort credit win; needs corporate email + credit card) | Rolling |
| 6 | **Microsoft Founders Hub** | Credits | Active | — | Entry tier ~$1–5k Azure+OpenAI; India/Nepal ok | Rolling |
| 7 | **AWS Activate** | Credits | Active | — | Needs business entity; check Nepal/India | Rolling |
| 8 | Ethereum Foundation ESP | Cash (crypto) | Active | — | Verify payout/tax stance | Rolling + RFPs |

## Submission log

| Date | Program | Action | Result / Reply | Follow-up |
|------|---------|--------|----------------|-----------|
| 2026-08-01 | (all) | Statuses re-verified; Protocol Labs found PAUSED; NLnet confirmed Taler/Fediversity-only until post-summer | — | Re-verify 2026-08-15 |

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
