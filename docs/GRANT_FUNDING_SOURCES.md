# Neuron Wire — Funding Sources & Strategy

**Document type:** Grant package annex · **Date:** 2026-07-31
**Applicant profile:** Solo founder, 16, Nepali citizen, high-school student (12th standard) in India · Project: Neuron Wire Protocol (NWP), MIT-licensed Rust codebase at [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) · Org identity: Zylvon

> **Verification note.** Every program below was checked against live sources (official pages, program
> announcements, and current news) on 2026-07-31. Amounts, deadlines, and eligibility change
> frequently — re-verify each program's page within two weeks of applying. Items that are paused,
> uncertain, or gated are flagged as such rather than listed as available.

---

## Applicant-wide eligibility notes (read first)

- **Age.** The applicant is 16. Several programs set an 18+ floor (GSoC, MLH, Outreachy, LFX, ISOC
  Youth Ambassador). Those are listed as *future* options with realistic timing. NLnet's NGI Zero
  programs **explicitly allow applicants under the age of legal consent with guardian consent** —
  the rare case where youth is a documented, supported path.
- **Residency vs. citizenship.** Most programs gate on *residency* (where you live / bank account),
  not citizenship. The applicant resides in India: India is supported by GitHub Sponsors, Microsoft
  for Startups, AWS Activate, and DigitalOcean; **Nepal is not on GitHub Sponsors' supported list**
  (confirmed via GitHub community discussions) — bank/payout location matters.
- **Legal capacity.** Grant agreements and credit-program terms are contracts. A minor generally
  needs a parent/guardian as co-signatory. Plan for this before applying; NLnet explicitly
  contemplates guardian consent in its negotiation process.
- **Legal entity.** Several startup programs (AWS Activate, Founders Hub tiers, Google for Startups,
  hardware programs) work best with a registered business. A simple sole proprietorship (India or
  Nepal) is cheap to register and sufficient for entry tiers. Registering Zylvon as a legal entity
  should be decided with a guardian/advisor, not improvised.
- **Payouts & tax.** Some grants (Filecoin, Ethereum ecosystem) pay in crypto or via crypto-native
  rails. Verify payout mechanics, local tax treatment, and student-visa income rules before
  accepting anything. When in doubt, prefer fiat-paying programs first.
- **One caution that applies to everything below:** nothing here is guaranteed. The strategy section
  explains how to make the portfolio robust to individual rejections.

---

## Tier A — Cash grants (the core of the "$5k cash" target)

### A1. NLnet Foundation (NGI Zero programs)
- **What it funds:** Open-source / open-internet infrastructure with public-interest impact — exactly
  NWP's category (decentralized infrastructure, privacy, trust). Programs: NGI Zero Core (general),
  NGI Zero Commons Fund (mature commons), NGI Zero Entrust (digital identity & trust — best thematic
  fit for NWP's security/trust layer), NGI Zero Discovery.
- **Amount range:** €5k–€50k typical (smaller grants common; NGI0 Commons Fund scales to ~€50k).
- **Eligibility / geography:** EU-based foundation (Netherlands). **No categorical exclusions** —
  "given equal proposals, inhabitants of the EU and Horizon Europe associated countries are given
  priority," and exceptional proposals from elsewhere with European relevance are eligible.
  **Applicants under legal consent age may apply; guardian consent required for negotiations.**
- **Application effort:** Medium — structured form (problem, approach, budget, open-source plan);
  bimonthly open calls, ~1 page of Q&A plus budget.
- **Status (2026-08-01):** ⚠️ **Temporary pause announced 2026-06-12** — NLnet is transitioning from
  NGI to an "Open Internet Stack" (OIS) program. NGI Zero Commons Fund's final call closed
  **2026-06-01** (no more applications for that fund); NGI Zero Core closed 2024. NLnet states the
  regular application process will **re-open after summer 2026 with 3 new programs under the OIS
  umbrella** (exception meanwhile: NGI Taler/Fediversity, not NWP-relevant). **Action: re-verify
  nlnet.nl/apply weekly; submission-ready draft at `docs/GRANT_SUBMISSIONS/NLNET_OIS.md` — submit
  within 2 weeks of reopening. This is the single best-fit cash source on this list.**
- **Submission-ready draft:** `docs/GRANT_SUBMISSIONS/NLNET_OIS.md` (prepared 2026-08-01: tagline,
  ~150-word abstract, work plan, €18,500 budget, team, sustainability).

### A2. Protocol Labs Research grants
- **What it funds:** Collaborative research on problems in decentralized computing (P2P, networking,
  distributed systems, verifiability) — NWP is squarely in scope.
- **Amount range:** Open grants typically $10k–$50k; targeted RFPs up to $150k (research) / $300k
  (implementation) per the program's published terms.
- **Eligibility / geography:** Worldwide; researcher-centric, individual-friendly.
- **Application effort:** High — research proposal with related work, methods, deliverables.
- **Status (2026-07):** Active (`research.protocol.ai/outreach/grants`). Note: Protocol Labs
  restructured its broader grants network in 2024–2025; the Research grant program is the current,
  live vehicle. Competitive; treat as a stretch application.

### A3. Filecoin Foundation developer grants
- **What it funds:** Development and data tooling, infrastructure, and research that advance a
  decentralized web — including decentralized storage/compute infrastructure like NWP.
- **Amount range:** "Next Step" grants $5k–$10k; open grants up to $50k; RFP track for scoped work.
- **Eligibility / geography:** Open internationally; builder- and research-friendly (grants platform
  at `fil.org/grants`, applications via `github.com/filecoin-project/devgrants`).
- **Application effort:** Low–Medium — short form + GitHub issue-style proposal; milestone-based.
- **Status (2026-07):** Active (Feb-2025 update confirms continued awarding). ⚠️ Crypto-native
  ecosystem — confirm payout form (FIL vs fiat) and local tax/legal position before applying.

### A4. Rust Foundation Community Grants
- **What it funds:** Work that benefits the Rust community — a pure-Rust, MIT-licensed infrastructure
  project like NWP is the target demographic. Program launched 2022 with a $625k budget across four
  grant categories (project, community, small, hardship).
- **Amount range:** Small grants typically a few hundred to a few thousand USD; project grants larger
  but competitive.
- **Eligibility / geography:** Worldwide; open to individual Rust community members.
- **Application effort:** Low–Medium — short proposal; small grants have lightweight review.
- **Status (2026-07):** Active; the Foundation also announced a Maintainers Fund (Nov 2025). Watch
  `rustfoundation.org/project-support` for the current call calendar.

### A5. Ethereum Foundation — Ecosystem Support Program (ESP)
- **What it funds:** Public goods, research, and infrastructure for decentralized systems. NWP fits
  the "infrastructure / research" category even without an Ethereum dependency — decentralized
  coordination is an ecosystem-relevant theme (see funded-projects list for scope).
- **Amount range:** Wide — $5k to $500k+ depending on scope; many grants in the $10k–$100k band.
- **Eligibility / geography:** Worldwide, open applications + RFPs.
- **Application effort:** Medium–High — application form + budget; RFP track is more structured.
- **Status (2026-07):** Active (ESP funded-projects list current through 2024+). ⚠️ Crypto-native
  ecosystem — same payout/tax verification as A3. Also worth monitoring: L2 ecosystem grant programs
  (Arbitrum, Optimism, Starknet) that periodically fund open infrastructure.

### A6. Internet Society Foundation
- **What it funds:** Research on the Internet and its impact on society; community connectivity
  projects; "Beyond the Net"-style small grants.
- **Amount range:** Research Grant Program 2026: $200k–$500k. Small/community grants are smaller.
- **Eligibility / geography:** **Organizations only** (universities, NGOs, research institutions) —
  not individuals. Worldwide geography for orgs.
- **Application effort:** High, and requires a host organization.
- **Status (2026-07):** 2026 Research Grant Program open. **Realistic path:** partnership with a
  university researcher (see C3) who can act as host institution — NWP's reproducible infrastructure
  is a credible research object for an Internet-society grant. Individual path: ISOC Youth Ambassador
  Programme (18–30) — eligible from 2028.

---

## Tier B — Structured programs & mentorship

### B1. Google Summer of Code (GSoC)
- **What it funds:** Stipends for contributors to open-source projects (3-month summer projects).
- **Amount range:** Stipend ~$1.5k–$9k depending on project size/region.
- **Eligibility / geography:** **Contributors must be 18+** — the applicant is ineligible as a
  contributor until 2028. Worldwide (non-embargoed countries).
- **Application effort:** Contributor: high (project proposal). **Org track:** NWP/Zylvon can apply
  as a *mentoring organization* — orgs need mentors (18+), an open-source community, and a credible
  project; this is a realistic 2027–2028 goal and turns the age constraint into a leadership
  position: the project would *pay students to build on NWP*.
- **Status (2026-07):** Annual cycle; org applications ~Feb, contributors ~Apr.

### B2. MLH Fellowship
- **What it funds:** Fully remote 12-week paid fellowships contributing to real open-source projects,
  with mentorship; year-round batches (open-source and software-engineering tracks).
- **Amount range:** Stipend-based (published per cohort).
- **Eligibility / geography:** Site states "anyone who meets these eligibility requirements is
  welcome to apply"; **historically 18+ with geography-dependent stipend eligibility** — verify the
  current cohort's terms.
- **Application effort:** Medium (application + technical assessment).
- **Status (2026-07):** Active site with year-round batches. ⚠️ Re-verify current eligibility page —
  the program was paused after 2023 and relaunched; terms differ by cohort. Realistic from 2028
  (age).

### B3. University research mentorship programs
- **What they fund:** Structured research internships / mentorship slots.
- **Formal programs (mostly 18+ or US-enrollment-gated):** US REU (requires US university
  enrollment), Outreachy (18+), LFX Mentorship (Linux Foundation, 18+), national schemes such as
  India's INSPIRE (school-student science awards — usually citizenship-restricted, check).
- **Realistic near-term path:** Direct research collaboration. Cold-email professors in distributed
  systems / P2P / ML (India: IITs, IISc, IIITs; plus any P2P lab anywhere) with a one-page summary
  and the reproducible repo as evidence. A 20,900-line, 342-test-passing, MIT-licensed codebase with
  one-command experiment reproduction is a strong cold-email. Even unpaid mentorship from one
  academic strengthens every other application on this list (and unlocks org-gated grants like A6).
- **Application effort:** Low (email) to Medium (formal programs).

---

## Tier C — Cloud credits & in-kind compute (the "$10k credits" target)

### C1. DigitalOcean Hatch
- **What it funds:** Cloud credits + partner perks for early-stage startups.
- **Amount range:** **$250 base credits (non-expiring)** via direct application; up to ~$100k via
  partner/accelerator bundles; a separate AI/ML track offers GPU compute for startups that have
  raised ≤ $10M.
- **Eligibility / geography:** Early-stage, pre-Series-A startups; available in most markets DO
  serves (India: yes).
- **Application effort:** Low — short form on `digitalocean.com/hatch`.
- **Status (2026-07):** Active. Base tier is the easiest credit win on this list; apply first.

### C2. AWS Activate
- **What it funds:** AWS credits + support for startups.
- **Amount range:** Founders tier ~$1k (direct application); $5k–$100k+ via accelerator/VC partners
  (Activate Portfolio).
- **Eligibility / geography:** Requires a startup (business entity); widely available — India: yes;
  Nepal: check local availability. Direct tier is accessible without an investor.
- **Application effort:** Low–Medium — online form; business verification.
- **Status (2026-07):** Active.

### C3. Microsoft for Startups Founders Hub
- **What it funds:** Azure credits, OpenAI credits, GitHub Enterprise, Microsoft 365, and support —
  tiered by startup stage.
- **Amount range:** Entry self-serve tier ~$1k–$5k in Azure credits; top tiers up to $150k Azure +
  OpenAI credits (higher tiers typically require investor-network referral).
- **Eligibility / geography:** Free to join; **launched in Nepal (Dec 2022) and available in 200+
  countries including India**. Requires business verification for credit tiers.
- **Application effort:** Low — online signup + verification; tier upgrades as the project advances.
- **Status (2026-07):** Active. Strong second credit win (Azure/OpenAI credits are useful for ML
  baselines and the benchmark suite).

### C4. Google for Startups (cloud credits)
- **What it funds:** Google Cloud credits + startup support.
- **Amount range:** $300 trial (everyone, no gating); Google for Startups Cloud Program up to
  ~$200k (higher figures reported for AI-first cohorts); often distributed via accelerators and
  regional Google for Startups hubs (India presence: yes; Nepal: limited).
- **Eligibility / geography:** Startups with a product; direct application possible in some regions,
  partner-gated in others.
- **Application effort:** Medium.
- **Status (2026-07):** Active. Treat as secondary to C2/C3 unless a regional hub or accelerator
  path opens.

### C5. Cloudflare for Startups
- **What it funds:** Cloudflare credits (Workers, CDN, WAF, R2, DDoS protection).
- **Amount range:** Self-serve tier ~$10k (bootstrapped startups, < $1M raised/revenue, < 10 years,
  public website + GitHub/social presence); up to $250k–$350k via partner tiers.
- **Eligibility / geography:** Global; direct application for the self-serve tier.
- **Application effort:** Low.
- **Status (2026-07):** Active. Honest scoping: funds NWP's *web layer* (metrics dashboard, seed
  node APIs, docs site) — not the P2P core. Still worth $10k of real infrastructure.

### C6. GitHub for Startups
- **What it funds:** GitHub Enterprise credits (20 seats) + up to $50k Actions credits for
  partner-accelerator startups.
- **Amount range:** $10k–$50k in value.
- **Eligibility / geography:** **Gated through partner accelerators/incubators** (YC, Techstars, 500,
  80+ partners) — not directly accessible today.
- **Application effort:** N/A until an accelerator relationship exists.
- **Status (2026-07):** Active but gated. Note: NWP already runs on GitHub Free with 4 CI workflows —
  the free tier is the current reality and it is sufficient.

### C7. Oracle Cloud Always Free (+ other free tiers)
- **What it funds:** Always-free compute: 2 AMD micro VMs + up to 4 OCPU ARM (Ampere A1) / 24 GB RAM
  + ~10 TB egress/month — enough for several long-running NWP nodes indefinitely.
- **Amount range:** $0 (free forever); requires a credit card for verification (no charges expected —
  keep spend within the free tier).
- **Eligibility / geography:** Worldwide.
- **Application effort:** Low — signup; ARM capacity can be scarce in some regions, retry.
- **Status (2026-07):** Active, but reports in Jul-2026 indicate the free ARM allocation is being
  quietly reduced for new accounts — check current terms; still the most generous free tier.
- **Also worth stacking (low effort):** Vultr new-account promos (~$100 one-time), Fly.io free
  allowances, Cloudflare Workers free tier, Vercel free tier (already hosting the NWP dashboard),
  GitHub Actions free minutes for public repos (already in use).

---

## Tier D — Security & privacy-specific funding

- **NLnet NGI Zero Entrust / Core** — see A1. Thematically the best fit for NWP's security layer
  (trust, identity, verifiability). EU-priority but worldwide-eligible.
- **Open Technology Fund (OTF)** — funds internet-freedom and security tools; **organization-focused
  (typically US-registered nonprofits)** — not realistic for this applicant now; revisit if a
  university/org partnership forms.
- **OpenSSF (Linux Foundation)** — free security tooling for OSS (Scorecard, Sigstore, best-practice
  badges) — no cash, but real security credibility; Alpha-Omega funding is org-only.
- **OSS-Fuzz** — Google's free fuzzing infrastructure for critical open-source projects. NWP already
  has 4 fuzz targets; integration is a concrete, fundable-by-nobody security win that raises the
  bar for the external audit story.
- **GitHub Security Lab** — free security research on critical OSS projects (they select projects).
  Worth nominating NWP after the WAN deployment adds real-world exposure.

---

## Tier E — Hardware donation programs (reality check)

Formal hardware donation programs overwhelmingly target **US/EU nonprofits and universities**:
Free Geek (US, local), PCs for People (US), TechSoup (nonprofits), Equinix Metal (via CNCF for
member orgs). None of these are realistically accessible to a solo 16-year-old in India today. The
practical equivalents, in order of usefulness:

1. **Oracle ARM free tier (C7)** — "virtual donated hardware": two permanent VPS-class nodes.
2. **Used/refurbished hardware in India** — the local second-hand market makes a 4–8 GB machine very
   cheap; one node is enough to run real experiments.
3. **Community-run nodes** — the decentralized thesis itself: as the testbed grows, volunteers donate
   compute. This is the only "hardware program" that scales with the project's mission.
4. **Later, with a legal entity:** NVIDIA Inception (AI-startup program — software/credits, not
   hardware), Equinix/CNCF-style programs, and university lab hosting via a research partnership
   (B3).

---

## Strategy — apply broad, stack, and don't wait

### The target stack

| Source | Type | Realistic value | Timing |
|--------|------|-----------------|--------|
| **Protocol Labs Research** | Cash | $10k–$50k | **ACTIVE — submit first** (draft: `GRANT_SUBMISSIONS/PROTOCOL_LABS.md`) |
| NLnet (OIS reopen) | Cash | €5k–€20k | Post-summer 2026 reopen (draft: `GRANT_SUBMISSIONS/NLNET_OIS.md`) |
| DigitalOcean Hatch | Credits | $250 (non-expiring) | **Apply first** — lowest effort |
| Rust Foundation Community Grants | Cash | $1k–$5k | Next cycle |
| Filecoin Foundation (Next Step) | Cash | $5k–$10k | Rolling |
| Microsoft Founders Hub | Credits | $1k–$5k (entry tier) | Immediate |
| AWS Activate (Founders) | Credits | ~$1k | Immediate |
| Cloudflare for Startups | Credits | ~$10k (self-serve) | Immediate |
| Oracle Always Free + free tiers | Compute | $0 permanent nodes | Immediate |
| **Total (conservative)** | — | **≈ $5k cash + ≈ $12k–17k credits + free compute** | 6–12 months |

The portfolio is deliberately **bimodal**: two or three small cash grants (achievable for an
individual with strong evidence) plus a stack of credit programs (high acceptance rate, low effort)
plus zero-cost compute. That combination funds the first WAN testbed without any single "yes."

### Rules of engagement

1. **Apply broad, early, and in parallel.** Credit programs (Tier C) are low-effort and largely
   independent — apply to all of them in the first two weeks. Cash grants have cycles — never miss a
   cycle waiting for a "better" application.
2. **Do not wait for one giant grant.** ISOC ($200k–$500k) and EF-scale grants are organization-level,
   slow, and unlikely to be a solo 16-year-old's first win. Treat them as a 2027+ play unlocked by
   the university partnership (B3), not as the plan.
3. **Prepare the logistics once, reuse everywhere.** (a) Guardian co-signature template; (b) one-page
   project summary + budget (the grant package already exists: GRANT.md, GRANT_RISKS.md, founder
   statement, audit, stats); (c) decision on registering a simple legal entity for Zylvon; (d) a
   payout plan (bank account, and an explicit decision on crypto payouts with a guardian/advisor).
4. **Let evidence compound.** Every grant that funds a WAN node produces data; every dataset
   strengthens the next application. The first $250 in DigitalOcean credits should be spent on the
   first *real* multi-node WAN experiment — the result of that experiment is the single most valuable
   artifact this portfolio can produce.
5. **Re-verify before every application.** Programs pause, reopen, and change terms (NLnet's 2026
   pause is the live example). Check the official page within two weeks of applying; this document
   is a map, not a guarantee.

---

*Supporting documents: [GRANT.md](../GRANT.md) · [GRANT_RISKS.md](GRANT_RISKS.md) · [GRANT_FOUNDER_STATEMENT.md](GRANT_FOUNDER_STATEMENT.md)*
