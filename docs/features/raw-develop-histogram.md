# Raw Develop histogram and scopes

A clean-room implementation of the analysis behind Raw Develop's SDR histogram and vectorscope,
based on Adobe's public documentation. It does not claim numerical identity with Adobe's
develop pipeline. The editor (`web/`) has no Raw Develop dialog; the filter is the engine command
`filter.rawDevelop`.

## PSD editable filters

Raw Develop is read and written as a live PSD editable filter (`Adobe Camera Raw Filter`, filter id
2783). The verified subset covers filter-relative Temperature/Tint with custom white balance,
Light, Texture/Clarity/Dehaze/Vibrance/Saturation, the parametric curve, four point curves,
all eight HSL bands, all four Color Grading wheels and Blending/Balance, sharpening,
noise amounts/detail, grain and the numeric post-crop vignette controls. Point curves contain
2–16 points with integer Input/Output levels; resetting one writes the linear endpoints.
PSD stores slider levels and curve coordinates as integers while Kition Photo's sliders are
continuous: PSD export rounds them to the nearest level (hues of 360° are written as 0°),
so a re-opened PSD shows whole numbers. Exposure and sharpening radius stay fractional.
The mapping comes from two revisions of a user-supplied Photoshop PSD written by Adobe Camera Raw
18.4 (process 6). Process 5's
corresponding version pair is also recognized; other versions remain opaque.

Imported filters keep their original descriptor in `params.__rawDevelopPsd`. Unchanged settings,
including the exact stored exposure double, are preserved; editing patches only changed mapped
fields. The template survives `.kphoto` and undo. Descriptors
with unrecognized processing fields remain `psd.unsupportedFilter`: their data survives a PSD
save, but their settings cannot be edited or re-rendered by Kition Photo. Non-default Kition Photo
settings without a verified PSD mapping cause the existing export warning and omission of that
filter; they are never silently written as default Raw Develop settings.

Active manual lens distortion/vignetting, luminance-noise contrast, color-noise smoothness,
post-crop vignette style (`PCVS`, still unverified) and highlights, RAW Kelvin white balance,
masks and HDR still need mapping and/or rendering support. Observed neutral-only controls are
retained in the template; non-neutral values keep the filter opaque. In the updated fixture,
Distortion −28, Lens Vignetting 28 and `PCVS=1` keep the entire filter opaque, while all settings
survive PSD and `.kphoto` saves. Its supported controls are also checked as a descriptor-only
projection with those three fields removed; that projection does not replace the source filter.

Matching settings does not
claim matching pixels: Kition Photo uses its own develop pipeline. PSD round trips and descriptor
patching are checked locally; reopening Kition Photo-generated exports in Photoshop is a separate
acceptance check.

## Layering and bounds

Analysis lives in L2 `kition-photo-algo` (`histogram`, `vectorscope`), colour transforms in L0
`kition-photo-cms`.

- The histogram counts each pixel with positive finite alpha once; transparent and non-finite
  pixels are excluded, and finite values outside 0–1 count in the edge bins and the overflow
  counters.
- The vectorscope bins hue and saturation into 128×64 cells; display orientation and colour
  management belong to its callers.
- Point curves in new `filter.rawDevelop` settings are validated: empty, or 2–16 finite points in
  0–255 with inputs at least one level apart. Stored editable filters are not validated on load, so
  a stored curve outside those limits still renders.

## Not covered

- HDR histograms, output and display: no suitable HDR hardware was available for validation.
  Float input and counters outside 0–1 do not imply HDR support.
- PSD Raw Develop settings outside the verified develop subset described above, and
  pixel-identical Adobe Camera Raw rendering.
- Raw Develop's own mask authoring.

References: [Adobe, Make color and tonal adjustments](https://helpx.adobe.com/camera-raw/desktop/using/make-color-tonal-adjustments-camera.html),
[Julieanne Kost, Adobe Camera Raw shortcuts and interactions](https://jkost.com/blog/2022/10/225-shortcuts-tips-and-tricks-for-adobe-camera-raw.html).
Only public behaviour was consulted; no Adobe code, shaders, profiles or assets are included.
