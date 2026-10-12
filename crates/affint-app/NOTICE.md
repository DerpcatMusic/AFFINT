# Native shell notices

MUI is used at git revision `c33c4451e8814562997171a1e00e9fb41fa955e6` from
https://github.com/Matari-Audio/MUI. Copyright (c) 2026 Matari Audio. MIT permission
and warranty notice: `licenses/MUI-MIT.txt`. AFFINT's host is independently authored
against the public MUI/Hybrid interfaces; it contains no copied vendor code.

Ubuntu Light font bytes come from `epaint_default_fonts` 0.34.3 and are distributed
under the Ubuntu Font Licence, retained in `licenses/Ubuntu-Font-Licence.txt`.
Copyright 2011 Canonical Ltd. Licensed under the Ubuntu Font Licence 1.0.
That crate is used as a bytes-only font source, with no egui UI dependency. No emoji
or third-party icon font is used. Original AFFINT paths supply icons and logo.

The native editing chrome embeds the unmodified installed Adwaita Sans Regular
font, the local Fontconfig fallback for Segoe UI. Its SIL Open Font License 1.1,
copyright notices and reserved-name terms are retained in
`licenses/Adwaita-OFL-1.1.txt`. Source: the system `adwaita-fonts` package at
`/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf`. No proprietary Segoe font is
redistributed. The Ubuntu bytes remain available to existing diagnostic fixtures.
The retained font SHA-256 is
`8381c33b9a44f066f2b99dba3d416a2342891e28c956a35dfd8d16ee2987e6d4`;
the retained OFL notice SHA-256 is
`459687971d21c53923c1d1c9c062ec273a7ea03226b36195b79ec6af7d98dc81`.

Vello Hybrid/Common, wgpu, Kurbo, arboard, winit, pollster, SHA-2 and their transitive
dependencies retain their package licenses and notices through the locked source
graph. Distribution must preserve applicable MIT/Apache notices. This file is not
a declaration that every transitive dependency has completed release-license audit.

Bytemuck 1.25.2 from https://github.com/Lokathor/bytemuck supplies safe
`try_cast_slice` borrowed F32 pixel views for native upload, with default features
disabled. Its MIT permission and warranty notice is retained in
`licenses/bytemuck-MIT.txt`; this adds no AFFINT-owned unsafe or full-frame copy.

rfd 0.17.2 is MIT licensed. Its pinned primary source and licence were checked at
https://github.com/PolyMeilex/rfd/tree/0.17.2. The installed desktop portal/fallback
environment is used; this project installs or changes no unrelated user software.

The synthetic study, original tool icons, AFFINT mark and display shader were
authored specifically for this project. They use no Affinity branding, artwork,
font assets or proprietary implementation.
