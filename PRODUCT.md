# AFFINT

<!-- impeccable:product-schema 1 -->

## Platform

Native desktop, Linux first. Windows/macOS portability is a design objective; their
first-release support remains an open decision. This native Rust desktop target
does not fit the web/mobile-only platform values in the current Impeccable template.

## Stack

Rust. Matari UI 0.3.2 at commit c33c4451e8814562997171a1e00e9fb41fa955e6 is selected
for the first native shell, using an AFFINT-owned winit host and shared wgpu 29.0.4
device with Vello Hybrid 0.2.0. Its intrinsic layout and public external texture
seam fit this workload. Coordinator color/clip/ordering proof passed on RX6600
Vulkan, with native display and keyboard/clipboard observations at 150% scaling. The earlier
eframe/egui-wgpu choice remains a researched fallback if that proof fails. Rendering
stays toolkit-independent; this choice makes no framework or imaging speed claim.

## Users

Artists and designers who work across vector drawing, photo editing, and page
layout, including the owner who currently runs the unified Affinity app through Wine.

## Product purpose

A unified editor with an Affinity-style comfortable workflow and shared document,
augmented with useful Photoshop masking and Illustrator/vector capabilities.
The first milestone is the shared document and canvas, followed by end-to-end
workflows rather than superficial feature/menu counts.

## Capabilities and constraints

Non-destructive editable operations, reversible atomic mutations, movable layer
relationships, reliable saves, and exact requested-resolution processing are
mandatory. Native Rust, idiomatic ownership, deep modules, audited reuse of useful
existing crates, and benchmark-backed optimization guide implementation. The
performance goal is ambitious; world-fastest status must never be asserted without
comparable measurements. Avoid Canva account/cloud/marketing clutter in the editor.

## Operating context

The unified Affinity installation is in the owner's local Wine prefix.
Research uses original synthetic documents, public
documentation, and the owner's explicitly authorized deep binary analysis. Raw
vendor output is isolated from implementors, who receive interface/behavior specs.
Open-source ArtCraft apps, Graphite, RAWmakase,
and other useful projects are references and reuse candidates, not automatically
trusted implementations or proof of parity.

Original AFFINT code and artwork are MIT licensed; fonts and dependencies retain
their own licenses as recorded in LICENSES.md.

## Brand commitments

Name: AFFINT. The owner requires faithful 1:1 editing-interface layout, density,
controls, studios, tool behavior and workflow against the installed unified
Affinity reference. A generic editor shell is explicitly rejected. Actual installed
UI definitions, runtime captures and saved UI settings govern reconstruction;
matching colours alone is insufficient. Keep AFFINT branding and audited assets.

## Product principles

- Preserve the original and keep operations editable.
- Correctness and honest errors underpin every performance improvement.
- A compact familiar editor lets the artwork lead.
- Drawing, dragging, zooming and panning must respond fluidly; passing headless
  tests does not establish interactive performance or acceptable UI fidelity.
- Use existing high-quality Rust crates where they solve the problem well.
- Compare actual completed workflows and faithful output, not menu counts.

## Open decisions

Broader interchange formats, advanced layout scope, minimum
hardware and distribution platforms require research or later
owner decisions. Local bounded CPU measurements exist; world-leading performance
and complete product parity are unproved. Raw vendor implementation stays in
restricted research; directly reused open-source code needs recorded provenance.
