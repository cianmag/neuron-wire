#!/usr/bin/env python3
"""Plot convergence data from a simulation run.

Usage: python plot_convergence.py <results-dir>
Example: python plot_convergence.py results/experiment_5node
"""

import sys
import csv
import os
import html

def load_convergence(path):
    """Load convergence.csv and return (ticks, node_names, peer_data)."""
    ticks = []
    node_names = []
    peer_data = []
    with open(path) as f:
        reader = csv.reader(f)
        headers = next(reader)
        node_names = headers[1:]  # skip 'tick'
        for row in reader:
            if len(row) < 2:
                continue
            ticks.append(int(row[0]))
            peer_data.append([int(x) if x else 0 for x in row[1:]])
    return ticks, node_names, peer_data


def load_summary(path):
    """Load summary.csv and return dict of key-value pairs."""
    result = {}
    with open(path) as f:
        reader = csv.DictReader(f)
        for row in reader:
            for k, v in row.items():
                result[k] = v
    return result


def generate_html(ticks, node_names, peer_data, summary, output_path):
    """Generate a self-contained HTML page with an SVG convergence chart."""
    
    node_count = len(node_names)
    total_samples = len(ticks)
    if total_samples == 0:
        print("No data to plot.")
        return
    
    max_peers = max(max(row) for row in peer_data) if peer_data else 0
    target_peers = int(summary.get('node_count', '0')) - 1
    
    # Find convergence time from summary
    converged = summary.get('converged', 'false') == 'true'
    conv_time = summary.get('convergence_time_secs', '')
    
    # Build SVG
    width = 900
    height = 500
    margin = {"top": 40, "right": 40, "bottom": 60, "left": 60}
    plot_w = width - margin["left"] - margin["right"]
    plot_h = height - margin["top"] - margin["bottom"]
    
    # Colors for each node
    colors = [
        "#ff6b6b", "#4ecdc4", "#45b7d1", "#f9ca24", "#6c5ce7",
        "#a29bfe", "#fd79a8", "#00cec9", "#e17055", "#0984e3",
        "#d63031", "#00b894", "#fdcb6e", "#e84393", "#6c5ce7"
    ]
    
    html_parts = []
    html_parts.append("""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Convergence Report</title>
<style>
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
         background: #0d1117; color: #c9d1d9; margin: 0; padding: 20px; }
  .container { max-width: 960px; margin: 0 auto; }
  h1 { color: #58a6ff; font-size: 24px; }
  h2 { color: #8b949e; font-size: 16px; font-weight: normal; }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); 
            gap: 12px; margin: 20px 0; }
  .stat-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; 
               padding: 16px; text-align: center; }
  .stat-value { font-size: 28px; font-weight: bold; color: #58a6ff; }
  .stat-label { font-size: 12px; color: #8b949e; margin-top: 4px; }
  svg { width: 100%; height: auto; }
  .converged-badge { display: inline-block; background: #238636; color: #fff; 
                     padding: 2px 12px; border-radius: 12px; font-size: 14px; }
  .not-converged { background: #da3633; }
</style>
</head>
<body>
<div class="container">
<h1>🌐 Convergence Experiment</h1>
""")
    
    # Stats cards
    conv_badge = f'<span class="converged-badge">✅ {conv_time}s</span>' if converged else '<span class="not-converged converged-badge">❌ Not converged</span>'
    
    html_parts.append(f"""
<div class="stats">
  <div class="stat-card">
    <div class="stat-value">{summary.get('node_count', '?')}</div>
    <div class="stat-label">Nodes</div>
  </div>
  <div class="stat-card">
    <div class="stat-value">{summary.get('duration_secs', '?')[:6]}s</div>
    <div class="stat-label">Duration</div>
  </div>
  <div class="stat-card">
    <div class="stat-value">{summary.get('total_packets_recv', '?')}</div>
    <div class="stat-label">Total Packets Recv</div>
  </div>
  <div class="stat-card">
    <div class="stat-value">{summary.get('avg_peers', '?')[:6]}</div>
    <div class="stat-label">Avg Peers</div>
  </div>
  <div class="stat-card">
    <div class="stat-value">{summary.get('max_peers', '?')}</div>
    <div class="stat-label">Max Peers</div>
  </div>
  <div class="stat-card">
    <div class="stat-value">{conv_badge}</div>
    <div class="stat-label">Convergence</div>
  </div>
</div>
<h2>Peer Discovery Over Time</h2>
""")
    
    # Build SVG
    svg_parts = [f'<svg viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg">']
    
    # Background
    svg_parts.append(f'<rect x="0" y="0" width="{width}" height="{height}" fill="#0d1117" rx="8"/>')
    
    # Grid
    svg_parts.append(f'<g stroke="#21262d" stroke-width="1">')
    # Y-axis gridlines
    for i in range(max_peers + 2):
        y = margin["top"] + plot_h - (i / (max_peers + 1) * plot_h) if max_peers > 0 else margin["top"] + plot_h
        y = margin["top"] + plot_h - (i / max(1, max_peers + 1) * plot_h) if max_peers >= 0 else margin["top"] + plot_h
        # Fix calculation
        y_val = i
        if max_peers > 0:
            y_pixel = margin["top"] + plot_h - (y_val / (max_peers + 1) * plot_h)
        else:
            y_pixel = margin["top"] + plot_h
        
        svg_parts.append(f'<line x1="{margin["left"]}" y1="{y_pixel}" x2="{width - margin["right"]}" y2="{y_pixel}" stroke="#21262d" stroke-dasharray="4,4"/>')
    
    # X-axis gridlines
    for i in range(0, total_samples, max(1, total_samples // 8)):
        x = margin["left"] + (i / max(1, total_samples - 1) * plot_w)
        svg_parts.append(f'<line x1="{x}" y1="{margin["top"]}" x2="{x}" y2="{height - margin["bottom"]}" stroke="#21262d" stroke-dasharray="4,4"/>')
    
    svg_parts.append('</g>')
    
    # Target line (node_count - 1)
    if max_peers > 0:
        target_y = margin["top"] + plot_h - (target_peers / (max_peers + 1) * plot_h)
        svg_parts.append(f'<line x1="{margin["left"]}" y1="{target_y}" x2="{width - margin["right"]}" y2="{target_y}" stroke="#238636" stroke-width="2" stroke-dasharray="6,3"/>')
        svg_parts.append(f'<text x="{width - margin["right"] + 5}" y="{target_y + 4}" fill="#238636" font-size="12">target ({target_peers} peers)</text>')
    
    # Plot each node's data as a stepped line
    for ni in range(node_count):
        if ni >= len(colors):
            continue
        color = colors[ni % len(colors)]
        points = []
        for si in range(total_samples):
            x = margin["left"] + (si / max(1, total_samples - 1) * plot_w)
            peer_val = peer_data[si][ni] if ni < len(peer_data[si]) else 0
            if max_peers > 0:
                y = margin["top"] + plot_h - (peer_val / (max_peers + 1) * plot_h)
            else:
                y = margin["top"] + plot_h
            points.append(f"{x:.1f},{y:.1f}")
        
        # Line
        svg_parts.append(f'<polyline points="{" ".join(points)}" fill="none" stroke="{color}" stroke-width="2" opacity="0.85"/>')
        # Label at last point
        if total_samples > 0:
            lx = margin["left"] + ((total_samples - 1) / max(1, total_samples - 1) * plot_w)
            lval = peer_data[-1][ni] if ni < len(peer_data[-1]) else 0
            if max_peers > 0:
                ly = margin["top"] + plot_h - (lval / (max_peers + 1) * plot_h)
            else:
                ly = margin["top"] + plot_h
            svg_parts.append(f'<text x="{lx + 5}" y="{ly + 4}" fill="{color}" font-size="11">{node_names[ni]}</text>')
    
    # Axes labels
    svg_parts.append(f'<text x="{width // 2}" y="{height - 8}" fill="#8b949e" font-size="13" text-anchor="middle">Time (tick)</text>')
    svg_parts.append(f'<text x="12" y="{height // 2}" fill="#8b949e" font-size="13" transform="rotate(-90, 12, {height // 2})" text-anchor="middle">Peers Discovered</text>')
    
    # X-axis tick labels
    for i in range(0, total_samples, max(1, total_samples // 8)):
        x = margin["left"] + (i / max(1, total_samples - 1) * plot_w)
        svg_parts.append(f'<text x="{x}" y="{height - margin["bottom"] + 18}" fill="#8b949e" font-size="10" text-anchor="middle">{ticks[i]}</text>')
    
    # Y-axis tick labels
    for i in range(max_peers + 2):
        if max_peers > 0:
            y_pixel = margin["top"] + plot_h - (i / (max_peers + 1) * plot_h)
        else:
            y_pixel = margin["top"] + plot_h
        svg_parts.append(f'<text x="{margin["left"] - 8}" y="{y_pixel + 4}" fill="#8b949e" font-size="10" text-anchor="end">{i}</text>')
    
    svg_parts.append('</svg>')
    
    html_parts.append('\n'.join(svg_parts))
    html_parts.append('</div></body></html>')
    
    html_content = '\n'.join(html_parts)
    
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    print(f"Report written to {output_path}")


def main():
    if len(sys.argv) < 2:
        print("Usage: python plot_convergence.py <results-dir>")
        sys.exit(1)
    
    results_dir = sys.argv[1]
    conv_path = os.path.join(results_dir, "convergence.csv")
    summary_path = os.path.join(results_dir, "summary.csv")
    
    if not os.path.exists(conv_path):
        print(f"Error: {conv_path} not found")
        sys.exit(1)
    if not os.path.exists(summary_path):
        print(f"Error: {summary_path} not found")
        sys.exit(1)
    
    ticks, node_names, peer_data = load_convergence(conv_path)
    summary = load_summary(summary_path)
    
    output_path = os.path.join(results_dir, "convergence_report.html")
    generate_html(ticks, node_names, peer_data, summary, output_path)


if __name__ == "__main__":
    main()
