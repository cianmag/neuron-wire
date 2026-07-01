# Neuron Wire Protocol — Website

This directory contains the standalone project website for the [Neuron Wire Protocol](https://github.com/cianmag/neuron-wire), a decentralized adaptive runtime for large-scale distributed learning.

## What's Here

| File | Purpose |
|---|---|
| `index.html` | Single-file, dark-themed landing page. No JavaScript dependencies. Works served from GitHub Pages, any static host, or opened directly in a browser. |

## Design

- **Theme:** Dark (#0a0a1a background) with cyan (#00c8ff) and purple (#7c3aed) accents
- **Typography:** System font stack with monospace for code blocks
- **Responsive:** Flexbox/grid layout adapts from wide desktop to narrow mobile
- **Zero JS:** Pure HTML + CSS, no frameworks, no build step

## Deployment

### GitHub Pages

To deploy as a GitHub Pages project site:

1. Push this directory to the `gh-pages` branch (or configure Pages to serve from `/website` on the main branch)
2. Access at `https://<org>.github.io/neuron-wire/`

### Any Static Host

Upload `index.html` to any web server or static host (Vercel, Netlify, S3, etc.).

### Local

Open `index.html` directly in any modern browser — no server required.

## Related

The project also has an [mdBook documentation site](https://github.com/cianmag/neuron-wire/tree/master/docs) with full architecture docs, protocol spec, developer guide, and research papers.
