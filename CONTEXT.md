# AFFINT document language

AFFINT combines vector, pixel, and page-layout work in one editable document.

## Language

**Document**: The complete editable artwork, its pages, layer relationships,
operations, and references to original assets.

**Asset**: An original source such as image pixels or a font resource whose content
is preserved independently of the edits applied to it.

**Layer**: A named visible or hidden document element that can be placed, ordered,
grouped, masked, or adjusted without destroying its original content.

**Live operation**: An editable transformation, filter, adjustment, boolean, or
crop whose settings and source relationship remain available after application.
_Avoid_: destructive effect when describing a live operation.

**Mask**: Editable coverage that controls where a layer or live operation contributes.
A mask changes visibility or effect coverage without deleting source content.

**Page**: A bounded layout region within the document that contains artwork and
layout relationships.

**Edit**: One meaningful user change that can be undone and redone as a whole,
including a continuous gesture after it is committed.

**Revision**: An identifiable committed state of the document used to distinguish
current artwork from earlier or pending work.

**Exact render**: Output evaluated at the requested pixel dimensions with the
declared color/precision policy, without hidden source or processing decimation.

**Interchange fidelity**: Preservation of document content and editability when
opening and saving files shared with another editor.
