#!/usr/bin/env python3
"""Build the Neuron Wire grant-package PDF from markdown docs.

Usage: python build_grant_pdf.py [outdir]
Deps: markdown, xhtml2pdf
"""
import html
import pathlib
import sys
import re

import markdown

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTDIR = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "release"
OUTDIR.mkdir(parents=True, exist_ok=True)

PACKAGE = [
    ("Technical Summary", "docs/TECHNICAL_SUMMARY.md"),
    ("Grant Summary (one page)", "docs/GRANT_SUMMARY.md"),
    ("Evidence Report (commit-pinned)", "docs/EVIDENCE_REPORT.md"),
    ("Budget", "docs/GRANT_BUDGET.md"),
    ("Roadmap & Milestones", "docs/GRANT_ROADMAP.md"),
    ("Risks & Mitigations", "docs/GRANT_RISKS.md"),
    ("Founder Statement", "docs/GRANT_FOUNDER_STATEMENT.md"),
]

MD_EXT = ["tables", "fenced_code", "sane_lists", "nl2br"]

CSS = """
@page { size: A4; margin: 1.6cm 1.7cm; @frame footer { -pdf-frame-content: footer; bottom: 0.8cm; height: 1cm; } }
body { font-family: Helvetica, Arial, sans-serif; font-size: 9.5pt; color: #1a1a1a; line-height: 1.45; }
h1 { font-size: 16pt; color: #0d3b66; border-bottom: 2px solid #0d3b66; padding-bottom: 3pt; margin: 14pt 0 8pt; }
h2 { font-size: 13pt; color: #0d3b66; margin: 12pt 0 6pt; border-bottom: 1px solid #c8d6e5; padding-bottom: 2pt; }
h3 { font-size: 11pt; color: #1b4f8a; margin: 9pt 0 4pt; }
p { margin: 4pt 0; }
ul, ol { margin: 4pt 0 6pt 14pt; }
li { margin: 2pt 0; }
table { border-collapse: collapse; width: 100%; margin: 6pt 0; font-size: 8.5pt; }
th { background: #0d3b66; color: white; text-align: left; padding: 3pt 5pt; }
td { border: 0.6pt solid #b0c4d8; padding: 3pt 5pt; vertical-align: top; }
tr:nth-child(even) td { background: #f0f5fa; }
code { font-family: "Courier New", monospace; font-size: 8pt; background: #eef2f6; padding: 0 2pt; }
pre { background: #f4f7fa; border: 0.6pt solid #c8d6e5; padding: 5pt; font-size: 7.5pt; white-space: pre-wrap; word-wrap: break-word; }
blockquote { border-left: 3pt solid #0d3b66; margin: 6pt 0; padding: 2pt 8pt; color: #444; background: #f7fafc; }
a { color: #0d3b66; text-decoration: none; }
hr { border: none; border-top: 0.8pt solid #c8d6e5; margin: 10pt 0; }
#footer { font-size: 7.5pt; color: #888; text-align: center; }
.pagebreak { page-break-before: always; }
"""


def md_to_html(path: pathlib.Path) -> str:
    text = path.read_text(encoding="utf-8")
    # strip local .md links' extension for readability in PDF
    text = re.sub(r"\]\(([^)#]+)\.md(#[^)]*)?\)", r"](docs/\1\2)", text)
    return markdown.markdown(text, extensions=MD_EXT)


def build() -> None:
    title = "Neuron Wire (NWP) — Grant Package"
    subtitle = (
        "Validated Research Prototype · v0.3.1 · MIT License · Zylvon<br/>"
        "github.com/cianmag/neuron-wire · Generated 2026-08-01"
    )
    body_parts = []
    for name, rel in PACKAGE:
        src = ROOT / rel
        body_parts.append(
            f'<h1 class="pagebreak">{html.escape(name)}</h1>\n'
            f'<p style="font-size:8pt;color:#888">Source: {rel}</p>\n'
            f"{md_to_html(src)}"
        )

    toc = "\n".join(
        f'<li><a href="#toc-{i}">{html.escape(name)}</a></li>' for i, (name, _) in enumerate(PACKAGE)
    )
    # patch first h1s to have ids
    for i, (name, _) in enumerate(PACKAGE):
        body_parts[i] = body_parts[i].replace(
            f"<h1 class=\"pagebreak\">{html.escape(name)}</h1>",
            f'<h1 class="pagebreak" id="toc-{i}">{html.escape(name)}</h1>',
            1,
        )

    html_doc = f"""<!DOCTYPE html><html><head><meta charset="utf-8"/><style>{CSS}</style></head><body>
<div id="footer">Neuron Wire Grant Package · v0.3.1 · github.com/cianmag/neuron-wire · page <pdf:pagenumber/> of <pdf:pagecount/></div>
<div style="text-align:center; margin-top: 30%;">
  <h1 style="border:none; font-size:26pt;">{title}</h1>
  <p style="font-size:13pt; color:#444;">{subtitle}</p>
  <hr style="width:60%;"/>
  <h2 style="text-align:center;">Contents</h2>
  <ul style="list-style:none; font-size:11pt;">{toc}</ul>
</div>
{''.join(body_parts)}
</body></html>"""

    out = OUTDIR / "neuron-wire-grant-package-v0.3.1.pdf"
    import xhtml2pdf.pisa as pisa

    with open(out, "wb") as f:
        status = pisa.CreatePDF(html_doc, dest=f, encoding="utf-8")
    if status.err:
        print(f"ERROR: {status.err}")
        sys.exit(1)
    print(f"OK {out} ({out.stat().st_size / 1024:.0f} KB)")


if __name__ == "__main__":
    build()
