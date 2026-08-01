#!/usr/bin/env python3
"""Generate demo-video slides (1280x720, dark theme matching the dashboard)."""
import pathlib
from PIL import Image, ImageDraw, ImageFont

OUT = pathlib.Path(__file__).resolve().parent / "slides"
OUT.mkdir(parents=True, exist_ok=True)

W, H = 1280, 720
BG = (13, 27, 42)        # #0d1b2a
BG2 = (20, 40, 60)       # #14283c
ACCENT = (0, 200, 255)   # #00c8ff
WHITE = (235, 240, 245)
GREY = (150, 165, 180)
GREEN = (80, 220, 120)

F_BIG = ImageFont.truetype(r"C:\Windows\Fonts\arialbd.ttf", 64)
F_TITLE = ImageFont.truetype(r"C:\Windows\Fonts\arialbd.ttf", 44)
F_SUB = ImageFont.truetype(r"C:\Windows\Fonts\arial.ttf", 28)
F_BODY = ImageFont.truetype(r"C:\Windows\Fonts\arial.ttf", 26)
F_BODY_B = ImageFont.truetype(r"C:\Windows\Fonts\arialbd.ttf", 26)
F_MONO = ImageFont.truetype(r"C:\Windows\Fonts\consola.ttf", 22)
F_SMALL = ImageFont.truetype(r"C:\Windows\Fonts\arial.ttf", 20)


def wrap(draw, text, font, max_w):
    words = text.split()
    lines, cur = [], ""
    for w_ in words:
        t = (cur + " " + w_).strip()
        if draw.textlength(t, font=font) <= max_w:
            cur = t
        else:
            if cur:
                lines.append(cur)
            cur = w_
    if cur:
        lines.append(cur)
    return lines


def new_frame():
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    d.rectangle([0, H - 6, W, H], fill=ACCENT)
    d.text((40, H - 44), "NEURON WIRE  ·  v0.3.1  ·  github.com/cianmag/neuron-wire", font=F_SMALL, fill=GREY)
    return img, d


def bullets(d, lines, x, y, font=F_BODY, gap=44, color=WHITE, marker="•"):
    for ln in lines:
        d.text((x, y), f"{marker} ", font=font, fill=ACCENT)
        d.text((x + 26, y), ln, font=font, fill=color)
        y += gap
    return y


# ── Slide 1: Title ──────────────────────────────────────────────
img, d = new_frame()
d.rectangle([0, 240, 10, 480], fill=ACCENT)
d.text((90, 250), "NEURON WIRE", font=F_BIG, fill=WHITE)
d.text((94, 340), "Infrastructure for Decentralized AI", font=F_TITLE, fill=ACCENT)
d.text((94, 420), "Peer discovery · gradient exchange · distributed learning — no central coordinator",
       font=F_SUB, fill=GREY)
d.text((94, 500), "Validated Research Prototype  ·  Rust  ·  MIT License  ·  v0.3.1", font=F_SUB, fill=WHITE)
d.text((94, 560), "342 tests passing on Linux CI  ·  commit-pinned evidence", font=F_SUB, fill=GREEN)
img.save(OUT / "s01.png")

# ── Slide 2: The problem ────────────────────────────────────────
img, d = new_frame()
d.text((60, 40), "The Problem", font=F_TITLE, fill=ACCENT)
d.rectangle([60, 110, 620, 112], fill=GREY)
d.text((60, 140), "Centralized AI concentrates power, trust, and cost", font=F_BODY_B, fill=WHITE)
bullets(d, [
    "One provider owns the model — users must trust it",
    "Training data must leave your device",
    "Edge devices are locked out of collective learning",
], 60, 210, gap=52)
d.text((60, 420), "Our answer", font=F_BODY_B, fill=ACCENT)
bullets(d, [
    "Any reachable device joins a P2P learning network",
    "Gradients — not data — travel between peers",
    "Latency-weighted Kademlia DHT + reliable UDP, no async runtime",
], 60, 480, gap=52)
img.save(OUT / "s02.png")

# ── Slide 3: Architecture ───────────────────────────────────────
img, d = new_frame()
d.text((60, 40), "Architecture — one auditable Rust codebase", font=F_TITLE, fill=ACCENT)
rows = [
    ("DHT routing", "latency-weighted Kademlia, 256 buckets, K=20, O(log N) convergence"),
    ("UDP transport", "3 reliability tiers, ACK bitfield, gradient aging, ~400 KHz–1 MHz ticks"),
    ("Identity + trust", "Ed25519 packet auth, optional XChaCha20-Poly1305, rate limiting"),
    ("Hebbian learning", "STDP, weight decay, sparse gossip, neurogenesis + apoptosis"),
    ("Simulator", "deterministic paper-mode, fixed seeds, known-good validation"),
]
y = 120
for name, desc in rows:
    d.rectangle([60, y, 60, y + 40], fill=ACCENT)
    d.text((90, y + 6), name, font=F_BODY_B, fill=WHITE)
    d.text((330, y + 8), desc, font=F_BODY, fill=GREY)
    y += 58
d.text((60, 470), "Single-threaded non-blocking engine · zero async-runtime dependency · runs on commodity hardware",
       font=F_SUB, fill=GREEN)
img.save(OUT / "s03.png")

# ── Slide 4: Evidence ───────────────────────────────────────────
img, d = new_frame()
d.text((60, 40), "Evidence — reproduced, not asserted", font=F_TITLE, fill=ACCENT)
rows = [
    ("342", "test functions pass on Linux CI"),
    ("E1–E9", "experiment matrix: churn, partition, malice, decay, trust"),
    ("100 K", "nodes converge 100% in 43 s (deterministic sim)"),
    ("25", "real processes converge on localhost UDP"),
    ("+23–25%", "robust neurogenesis effect on bandwidth (E9)"),
    ("7", "pipeline-caught bugs documented as negative results"),
]
x0, y0 = 60, 120
for i, (num, label) in enumerate(rows):
    col = i % 3
    row = i // 3
    x = x0 + col * 400
    y = y0 + row * 215
    d.rectangle([x, y, x + 350, y + 185], fill=BG2)
    d.text((x + 20, y + 18), num, font=F_BIG, fill=ACCENT)
    for j, ln in enumerate(wrap(d, label, F_SMALL, 310)):
        d.text((x + 20, y + 105 + j * 27), ln, font=F_SMALL, fill=WHITE)
img.save(OUT / "s04.png")

# ── Slide 5: Distributed learning E2E ───────────────────────────
img, d = new_frame()
d.text((60, 40), "Distributed Learning — end to end", font=F_TITLE, fill=ACCENT)
flow = [
    "Node A fires an activation frame over real UDP",
    "Node B decodes it → Hebbian STDP updates a synapse: 0.5 → >0.6",
    "B sends the learning signal back to A — bidirectional",
    "Same seed → same weight change (deterministic)",
]
y = 120
for i, ln in enumerate(flow):
    d.ellipse([60, y + 8, 84, y + 32], fill=ACCENT)
    d.text((100, y + 6), ln, font=F_BODY, fill=WHITE)
    y += 56
d.rectangle([60, 380, 1220, 470], fill=BG2)
d.text((80, 395), "This test caught a real production bug:", font=F_BODY_B, fill=WHITE)
d.text((80, 430), "Adam optimiser produced NaN on its first tick — fixed with regression tests (a02b241)",
       font=F_MONO, fill=GREEN)
img.save(OUT / "s05.png")

# ── Slide 6: The ask ────────────────────────────────────────────
img, d = new_frame()
d.text((60, 40), "The Funding Request — $20,000", font=F_TITLE, fill=ACCENT)
bullets(d, [
    "20-node, three-region pilot (NA / EU / APAC), 7-day sustained mesh",
    "External security audit of cryptographic + transport layers",
    "Independent reproduction; comparison vs FedAvg and decentralized SGD",
    "Publication: reproducible datasets + peer-reviewed paper",
], 60, 130, gap=58)
d.text((60, 400), "Budget", font=F_BODY_B, fill=ACCENT)
d.text((60, 440), "cloud $4k · hardware $3k · security review $5k · compute $3k · services $1k · docs $1k · contingency $3k",
       font=F_BODY, fill=WHITE)
d.text((60, 500), "Alternative packages: $10k and $5k — same milestones, scaled pilot",
       font=F_SUB, fill=GREY)
img.save(OUT / "s06.png")

# ── Slide 7: Roadmap ────────────────────────────────────────────
img, d = new_frame()
d.text((60, 40), "Roadmap", font=F_TITLE, fill=ACCENT)
rows = [
    ("M1", "Green CI + verified alpha — 342 tests", True),
    ("M2", "Reproducible 100-node simulation", True),
    ("M3", "Local 25-process test", True),
    ("M4", "Network emulation benchmark", True),
    ("M5", "Funded 20-node, three-region pilot — 7-day mesh", False),
    ("M6", "Publication + external security audit", False),
]
y = 120
for m, desc, done in rows:
    color = GREEN if done else GREY
    mark = "✓" if done else "→"
    d.text((60, y), m, font=F_BODY_B, fill=ACCENT)
    d.text((130, y), mark, font=F_BODY_B, fill=color)
    d.text((170, y), desc, font=F_BODY, fill=WHITE if done else GREY)
    y += 62
d.text((60, 500), "Next milestone: secure distributed-learning E2E + grant submission — not more features.",
       font=F_SUB, fill=GREEN)
img.save(OUT / "s07.png")

# ── Slide 8: Close ──────────────────────────────────────────────
img, d = new_frame()
d.rectangle([0, 240, 10, 480], fill=ACCENT)
d.text((90, 260), "One command reproduces every number", font=F_TITLE, fill=WHITE)
d.text((94, 350), "git clone https://github.com/cianmag/neuron-wire", font=F_MONO, fill=ACCENT)
d.text((94, 400), "cd neuron-wire && cargo test   →   342 passed", font=F_MONO, fill=ACCENT)
d.text((94, 480), "Evidence report with commit-pinned runs: docs/EVIDENCE_REPORT.md", font=F_SUB, fill=GREY)
d.text((94, 540), "Zylvon · grant package in docs/GRANT_* · MIT License", font=F_SUB, fill=WHITE)
img.save(OUT / "s08.png")

print("slides:", [p.name for p in sorted(OUT.glob("*.png"))])
