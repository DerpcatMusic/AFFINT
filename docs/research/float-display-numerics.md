# Floating display numeric follow-up

2026-10-12. Source inspection and independent scalar analysis; this report does
not establish an observed adapter failure or a passing new AFFINT/GPU test.
The corresponding renderer and native acceptance tests await coordinator execution.

The ordinary F32 frame guarantees canonical zero-alpha RGB. Its display shader
can therefore use the premultiplied RGB directly, without making its contribution
conditional on an alpha comparison. The existing controlled half-float diagnostic
also supplies canonical pixels, including `[0,0,0,0]` for its zero-alpha sample.

This matters for valid tiny-alpha artwork. A full-coverage white vector with alpha
`2^-130` followed by four separate `Exposure(32)` operations produces these exact
power-of-two working components:

```text
RGB = 2^-130 × 2^32 × 2^32 × 2^32 × 2^32 = 0.25
alpha = 2^-130, f32 bits 0x00080000
```

Each exposure gain and resulting RGB remains finite. Alpha is positive and
subnormal; the premultiplied RGB is normal and visibly contributes to the final
opaque backdrop. Collapsing the gains into a single f32 product would overflow
and is not equivalent to the ordered operations.

WGSL permits subnormal inputs/results of its floating arithmetic, comparisons and
several built-ins to become zero. Consequently, an alpha-based RGB guard can lose
this contribution on a conforming backend. This is a portability inference, not
a measured RX 6600 defect. [Dated WGSL floating-point rules](https://www.w3.org/TR/2026/CRD-WGSL-20260921/#floating-point-evaluation).

Independent double-precision sRGB calculations predict the following final codes
after composition. They use the declared backdrop shades and no AFFINT shader:

| Checker sRGB | RGB discarded if alpha becomes zero | Premultiplied RGB retained |
| --- | --- | --- |
| 0.78 | 199 | 234 |
| 0.90 | 230 | 255 |

Scalar inputs, alpha bits, formulas' results and the primary URL are retained in
`.research/float-display-numerics/scalar-oracle.json`. Native acceptance must cross
the real document, F32 upload and converter, preserve ordinary canonical-zero
fixtures, and agree with the independent final display within one code. This
limited display decision does not solve arbitrary GPU document arithmetic,
subnormal intermediates, overflow or coordinate precision.

The same review identified an existing RGBA8 output-policy gap: a positive working
alpha that quantizes to byte zero could retain nonzero encoded RGB. Computing the
quantized alpha first and emitting `[0,0,0,0]` in that case implements the already
declared canonical transparency rule. A white vector at alpha `1e-4` is the new
independent regression. Original assets, editable alpha/operations and native
storage remain intact; explicit RGBA8 export is still lossy SDR output. This is a
source-established correction with new acceptance coverage, not a claim that the
coordinator ran its pre-fix version. Prior supported golden fixtures remain required.
