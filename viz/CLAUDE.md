# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This directory holds `template.html`, the HTML/CSS/JS template for the interactive "fitness landscape" visualization exported by the parent Rust project's `viz` subcommand (see `../CLAUDE.md` for the overall project). It's not a standalone web app — no build step, no package manager, no server. It's a static template that gets data-injected by Rust and then opened directly in a browser.

## Relationship to the parent project

- `../src/viz.rs` embeds this file at compile time via `include_str!("../viz/template.html")`.
- Running the `viz` subcommand from the project root:
  ```bash
  cargo run --release -- viz --output landscape.html --players 2,3,4 --restarts 60
  ```
  runs instrumented hill-climbing / beam-search experiments in Rust (needs the precomputed `hands` table — see `../CLAUDE.md`), serializes the results to JSON, and replaces the `/*__LANDSCAPE_DATA__*/` placeholder in this template with `const DATA = {...};`, writing the result to the output path (e.g. `landscape.html`).
- To iterate on the template's visuals or layout: edit `template.html`, then re-run the `viz` command from the project root and open the freshly generated output file in a browser. There is no live-reload — each edit requires a fresh Rust export, since data injection happens at export time, not at page load.
- Opening `template.html` itself directly in a browser will not render charts: the `/*__LANDSCAPE_DATA__*/` placeholder is only replaced in the generated output, so `DATA` is undefined until that happens.

## Structure of template.html

- **`<style>`**: theming is done entirely via CSS custom properties, defined in three places that must be kept in sync: `:root` (light, default), `@media (prefers-color-scheme: dark)`, and explicit `:root[data-theme="dark"]` / `:root[data-theme="light"]` overrides (driven by the in-page theme-toggle button, which takes precedence over the OS preference). Per-player-count color identity uses `--n2` / `--n3` / `--n4` (blue / amber / red); diverging win/loss colors use `--pos` / `--neg`.
- **Data placeholder**: `/*__LANDSCAPE_DATA__*/` — see above.
- **Rendering**: plain DOM/SVG, no framework or charting library. The page has 9 numbered `<section>`s (trajectories, optima histogram, neighbor profile, collapse curve, deck inspector, ridge walk / topology, climb-as-graph, mutation operators, beam search), each with a `render*()` function that clears its host `<div>` and redraws an inline SVG from scratch.
- **Shared helpers**: `svg()` / `el()` / `text()` build SVG nodes; `css()` reads a CSS custom property at render time; `showTip()` / `hideTip()` drive the single shared `#tt` tooltip element; `buildSeg()` wires up the per-section player-count toggle buttons (`.seg` control groups); `niceTicks()` computes axis tick values.
- **`rerenderAll()`** re-runs every section's `render*()` function. It's called on init and whenever the theme toggles, since colors are read live from CSS variables at draw time rather than being CSS-only — an SVG redraw is required to pick up new colors.

## Data contract

The shape of `DATA` is defined by the `#[derive(Serialize)]` structs in `../src/viz.rs` (`Landscape { meta, players: Vec<PlayerData> }`). The contract is one-directional: Rust produces the JSON, the JS in this template only consumes it. If a chart needs a new field, add it to the relevant Rust struct and populate it in `build_player_data` (or `export`) in `viz.rs`, not in the template.
