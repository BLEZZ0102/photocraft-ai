# Layer references for Codex image editing

The desktop **AI Edit** dialog can send the visible canvas plus individual layer
previews and a JSON manifest to the Codex provider using ChatGPT sign-in. This is
input context for instructions such as “change the object on Layer 2”; model
interpretation is not guaranteed. The returned image is still one full-canvas
result added as a new top-level layer with one undo step. Source layers stay intact.

## Sharing controls

- **Include individual layer images, names and structure** defaults to on.
- **Also send hidden layers** defaults to off for every new edit. This includes
  layers inside hidden groups only when explicitly enabled. Turning off individual
  references also clears hidden-layer sharing.
- Turning off individual references sends only the visible canvas and instruction.
- These controls apply to Codex only. Gemini, Seedream and FLUX keep their existing
  flattened-canvas input protocols; they do not silently receive layer metadata.

“Visible” follows the layer and ancestor visibility switches. Isolated previews
can reveal content covered by other visible layers or group masks. A fully transparent or locally masked
layer can produce an empty preview. No hidden names, IDs or hierarchy are included
without opt-in. A selected hidden layer is not an exception.

## Context format and limits

The manifest lists included layers top to bottom, with unique per-request reference
labels, document layer IDs, exact names, parent references, kind, visibility,
the active-layer flag, opacity, fill opacity, blend mode, clipping and mask-presence flags.
Duplicate names can be distinguished by reference label or ID. Only preview images
and the manifest are sent, not a PSD or a serialization of the native document.

Raster, text, shape, fill and smart-object leaf layers receive full-canvas isolated
PNG previews at their original canvas position. Masks, local opacity and effects
are rendered. The preview uses normal blending and removes clipping to show the
isolated layer; group effects, backdrop blending, clipping and adjustment effects
must be interpreted using the canvas reference. Groups and adjustment layers have
metadata only. Internal smart-object documents and raw editable text/style data are
not exported. The source document is never modified to render these references.

Previews use the document's colour context, are converted through PhotoCraft CMS
to sRGB and encoded as 8-bit PNGs. The source document keeps its mode and depth.
Export documents omit EXIF, XMP, original file names and opaque PSD blocks. Layer
names are included only in the manifest, never used as filesystem paths. The
provider identifies layer metadata as document data rather than instructions.

PhotoCraft's own per-request budgets are 15 leaf images plus the canvas, 128 included
layer/group entries, 1024 UTF-8 bytes per layer name, 64 KiB of manifest and 64 MiB
of PNG inputs in total. These are application limits, not claims about model limits.
The existing 16-megapixel canvas limit remains. Trees are bounded to 4096 entries
and 32 levels before snapshotting or rendering. Oversized requests fail clearly
before calling the provider, rather than dropping references. References render
one at a time, with cancellation checks between renders, and temporary files are
removed when the job ends.

## Engine parameters

`edit.ai.codex` accepts:

```json
{
  "prompt": "Change the object on Layer 2 to yellow",
  "sendLayerContext": true,
  "includeHiddenLayers": false
}
```

Both sharing parameters must be booleans. Hidden-layer sharing requires individual
references. The same defaults apply to GUI, CLI and control-channel calls. A
provider that does not implement layer references returns an explicit unsupported
error; it must never fall back silently to flattened input.

Offline regression tests cover hidden ancestors, ordering and duplicate names,
isolated image content, colour/depth conversion, size limits, cancellation,
provider input assembly, source preservation, undo/redo and dialog sharing controls.
Live model quality and installed-runtime multi-image behaviour require a separate
signed-in generation check; offline tests do not establish those outcomes.
