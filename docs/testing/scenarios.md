# Kition Photo scenario regression suite

The tests run against Kition Photo alone. They do not launch, install, locate, read, or download
any other application. Expected colors, dimensions, names, text and history behavior
are written in the cases. No test regenerates an expected image from the current implementation.

## Run locally

Required sibling checkout: `kition-fonts` (the CJK test face; or set `KITION_FONTS_DIR`).
The web packages (`@kition/sdk`, `@kition/tessera`) come from npm, locked by
`web/pnpm-lock.yaml`; with `KITION_LINK=1`, `web/.pnpmfile.cjs` links `kition-sdk` and
`kition-tessera` checkouts instead (next to this repository, or `KITION_SDK` and `KITION_TESSERA`).

```text
workspace/
  kition-photo/
  kition-fonts/
  kition-sdk/        (optional, for KITION_LINK=1 development)
  kition-tessera/    (optional, for KITION_LINK=1 development)
```

Use Node 22.19+, pnpm 10, Rust stable >=1.95, wasm-pack and the wasm32-unknown-unknown target.
On macOS use rustup's Cargo/rustc rather than a different Homebrew compiler earlier in PATH.
Apple's archiver may reject the wasm `psm` archive; use LLVM's clang/llvm-ar if that occurs.
Keep a linked SDK source stable during a browser run: concurrent edits can trigger Vite reloads
and replace test documents with the development sample. CI installs the published packages, as a
default install does; that is also suitable for local validation when the SDK is being edited elsewhere.

```sh
pnpm --dir web install --frozen-lockfile
rustup target add wasm32-unknown-unknown
pnpm --dir web engine
pnpm --dir web exec playwright install --with-deps chromium webkit
pnpm --dir web test:ci
pnpm --dir web test:ci:full
```

`test:ci` runs the production typecheck/build, existing parity/unit checks, all engine scenarios,
and Chromium scenarios. `test:ci:full` also runs WebKit scenarios and the existing broad editor
flow in both browsers. The build step must precede these commands: the CI workflow always builds
the current source, and the test preflight rejects missing WASM or CJK font assets.

For development:

```sh
pnpm --dir web test:scenarios:engine
pnpm --dir web test:scenarios:ui
pnpm --dir web exec playwright test --project=chromium --grep UI04
pnpm --dir web exec playwright test --headed
pnpm --dir web dev --host 127.0.0.1
# Open http://127.0.0.1:5187/dev.html
```

## Scenarios and acceptance criteria

Every engine case starts with a fresh white 128×96 document unless specified otherwise.
Fixtures are generated from original geometric primitives, solid colors and short text in
`web/tests/scenarios/cases.json`; no external photo corpus is required for these scenarios.
Every editor browser test gets a fresh storage context and a PSD fixture through the host's real file
input. Actions under test use pointer/keyboard/control interactions, with engine inspection as
an independent observation channel. Browser color checks read the displayed canvas pixels before
sampling engine output, since an agent sampling command can trigger a repaint and hide stale pixels.

| Scenario | Priority | Operation sequence | Assertions | Browser test |
|---|---|---|---|---|
| SC01 Poster | P0 | Rectangle banner → title → edit text → save PSD → reopen/edit → native save | Shape/type layers, red background/white margin, editable text, identical composite and layer properties | UI05 |
| SC02 Photo preparation | P0 | Flat color photo → crop → nearest resize → invert → PNG → reopen | 80×64 then 40×32; exact complement color; export dimensions and pixels | UI04 (crop/cancel/undo) |
| SC03 Local editing | P0 | Rectangular selection → red fill → inverse → blue fill → undo/redo → deselect → PSD | Selection bounds, inside/outside/boundary pixels, exact undo/redo and saved pixels | UI02 |
| SC04 Composite | P0 | Red layer → Multiply/50% opacity → selection mask → PSD → disable mask → undo → native save | Layer properties, mask visibility, composite colors, preservation after reload | UI03 (40% opacity) |
| SC05 Painting/history | P0 | Seeded black stroke → erase crossing → undo/redo → undo → rename → PSD | Painted/untouched pixels, full-image history restoration, redo branch cleared | UI01 |
| SC06 Chinese title | P1 | Create Chinese → replace by Unicode character range → style → compare implicit/explicit CJK fallback → PSD → edit again | Text, 26 pt size, first-load glyph rendering, pixels and editable text after reload | UI05; Chromium real IME composition, WebKit text entry |
| SC07 PSD second edit | P0 | Save layered source → reopen → duplicate → translate → hide → undo → save/reopen → delete copy | Original pixels retained, translated copy at expected location, layers preserved | Engine coverage; UI save/reopen also exercised by UI01–07 |
| SC08 Error recovery | P0 | Edit → unknown command → invalid crop → malformed PSD open → continue editing → save | Rejections, unchanged revision/history/document, subsequent edit succeeds | UI06 covers rasterization cancel/confirm/history |
| SC09 Mask painting | P1 | Red layer → reveal-all mask → paint black on mask → disable mask → undo → PSD | Mask hides center; disabling reveals intact red source; reload preserves result | Existing broad flow covers mask target controls |
| SC10-8/16/32 Bit depth | P1 | New document at each depth → fill → invert → native save/reopen | Depth and size retained; expected output pixels | Engine coverage |
| SC11 Isolation | P0 | Edit A → edit/undo B → inspect/save A → mark A saved | Pixels, history and dirty states do not leak between documents | UI07: two host tabs, separate files and undo |
| SC12 First CJK font (two variants) | P0 | Fresh WASM → render without CJK → register once → horizontal/vertical Chinese → explicit font → undo → PSD reopen/edit | First registration replaces missing glyphs; default Inter fallback matches explicit Droid Sans Fallback pixels; history and editable PSD preserve text | Engine coverage with a separate WASM instance per variant |

Engine save/reopen comparisons check full RGBA buffers and stable layer properties, not just file
signatures or “command succeeded.” PSD quantizes opacity to 8 bits, and a document named `.psd`
keeps layer opacity as the file stores it from the first edit: SC04 alone allows one output
channel level and 1/255 opacity error (a step's `tolerance.opacity`) when it sets 50% opacity and
at the PSD conversion. A new checkpoint after conversion makes
subsequent undo/native saves exact again. PNG checks compare pixels/dimensions, since flat exports
intentionally discard the layer tree. Scene text uses the pinned font input, never system fonts.
SC12 uses independent WASM instances so earlier font registrations cannot hide a first-load bug.
Its no-font, default-fallback and explicit-font PNGs are retained under
`test-results/engine/SC12-horizontal/` and `SC12-vertical/`, including on failure.

Additional interaction regressions found during the follow-up review:

| Browser scenario | Operation sequence | Assertions |
|---|---|---|
| UI08 (three variants) | Type a draft → IME Enter/Escape or legacy key code 229 → finish Chinese text → PSD save/reopen | Candidate keys preserve the draft and create no layer; ordinary Enter creates exactly one editable title |
| UI09 | Configure 40×24 fixed-size marquee → click without dragging → paint across it → PSD save/reopen | Exact selection bounds; only inside pixels change |
| UI10 (two variants) | Commit title with Enter or toolbar → immediately undo/redo → PSD save/reopen | Undo removes the title; redo restores exactly one editable layer |
| UI11 | Type title → click zoom field → change zoom → PSD save/reopen | Blur commits the title without stealing focus from the zoom field |
| UI12 | Type title → Escape → Brush shortcut | No text layer or revision change; keyboard tool selection still works |
| UI13 | Draw a red rectangle with a 20 px black stroke → undo/redo → PSD save/reopen | Displayed and engine pixels outside the drag box include the stroke; fill and history remain correct |
| UI14 | Press Brush → release at a newer position without an intervening move → undo/redo → PSD save/reopen | Middle and endpoint pixels are painted |
| UI15 | Press Move on a shape → release at a newer position → undo/redo → PSD save/reopen | Old location clears; the shape moves by the full release displacement and stays there after reopening |
| UI16 (two variants) | Select two distant shapes, or link them and select one → Move → undo/redo → PSD save/reopen | Both layers clear their old displayed positions and appear at the new positions; inspect the non-active layer before any engine sampling |
| UI17 (two variants) | Add an outer stroke to a pixel layer or its parent group after choosing Brush → paint → undo/redo → PSD save/reopen | The blue style outside the brush bounds appears immediately; red ink and untouched white margin remain correct |
| UI18 (four variants) | Rectangle/ellipse, normal/fixed ratio → select → zoom to 200% → click at a fractional document point → paint → undo/redo → PSD reopen | Click clears the selection; paint reaches both sides outside the former selection |
| UI19 | Scrubby Zoom press → release 100 screen pixels to the right without a move event | Zoom becomes 200%; document revision is unchanged |
| UI20 | Create editable text → choose Brush → hide Color group → inspect Properties at its top | Character heading is fully visible without scrolling in the default dock layout |
| UI21 (four variants) | Fixed-size 40×24 rectangle/ellipse → zoom to 200% → click or drag up-left from a fractional point → paint → undo/redo → PSD reopen | Exact 40×24 selection; the first pixel beyond its right edge stays white before and after saving |
| UI22 (six variants) | Rectangle/ellipse with fixed 1:1, reversed 3:2 or Shift square → 200% drag from mixed fractional coordinates → paint → undo/redo → PSD reopen | Exact constrained dimensions; outside pixels remain white and inside pixels persist across history and saving |
| UI23 (six variants) | Rectangle/ellipse/lasso × Add/Subtract → empty click at 200%, toolbar then Shift/Alt override → undo/redo selection → paint → PSD reopen | Selection and document revision stay unchanged; no extra undo step; painting stays inside the original selection |
| UI24 (three variants) | Overlapping rectangular selections with Add/Subtract/Intersect → undo/redo → paint across both → undo/redo painting → PSD reopen | Exact union, cutout or overlap pixels, including pixels inside the bounding box but outside the mask |
| UI25 (ten variants) | Rectangle/ellipse shape, Crop, Slice or Object Selection → visible preview → sparse release at press or pointer cancellation → paint → undo/redo → PSD reopen | Transient canvas becomes transparent before any repaint; document revision and structure stay unchanged; later painting and history work |
| UI26 (two variants) | Commit crop with Enter while its worker request is held → immediate editor focus or focus another option → release real request → undo/redo → PSD reopen | Shortcuts remain reachable before the reply; completion does not steal a later focus choice; crop history and saved dimensions remain correct |
| UI27 (three variants) | Cancel a crop by click, Enter or Escape while Cancel is focused → Brush shortcut → keyboard undo/redo → PSD reopen | Cancel changes neither revision nor document structure; the disabled button does not swallow shortcuts; prior painting remains recoverable |
| UI28 (four variants) | Commit text by Enter or toolbar while its real worker request is held → immediate shortcut focus or edit zoom → release → undo/redo → PSD reopen | Exactly one text request and layer; focus is available before the reply and is not stolen from the zoom field on completion |
| UI29 (six variants) | Horizontal/vertical text draft → direct tool switch, visit font size, or empty draft with font size → Move → undo/redo → PSD reopen → fresh draft/cancel | Old draft closes; one text layer preserves text, size, orientation and transform across history/save; empty drafts add no revision; the next draft starts empty |
| UI30 (six variants) | Horizontal/vertical draft → second canvas click, directly or after font-size editing, with nonempty/empty first draft → keyboard type → undo/redo → PSD reopen | New input is empty and focused; first text stays at its original anchor; each nonempty draft creates exactly one layer/history step; text, size, orientation and both anchors survive saving |
| UI31 (four variants) | Horizontal/vertical draft → wheel pan/zoom → continue typing → commit → undo/redo → PSD reopen | Input baseline follows an independently observed rendered marker; focus/text stay intact; view changes do not edit the document; committed anchor, 48 pt size and orientation persist |
| UI32 (six variants) | Horizontal/vertical nonempty/blank draft → save shortcut; hide/disable Save → attempt shortcut → allow Save | First written PSD includes exactly one editable title with the original anchor, size and orientation; blank drafts add no history; denied saving preserves text/focus; undo/redo and reopen remain correct |
| UI33 (ten variants) | Horizontal/vertical draft → host denies type creation → Enter/Save/tool switch/new canvas click → allow creation → continue typing and retry Save | Refused text stays editable at the original anchor; no document edits or file writes; tool/new-draft transitions stop; retry creates exactly one title with correct size/orientation, undo/redo and first-write PSD persistence; two pending-command variants check that the last tool/canvas intent wins |
| UI34 (eight variants) | Horizontal/vertical draft → edit font size → toolbar Save from clean/dirty document; reject/retry; blank/cancel; pending text with repeated saves | Save enables for nonblank drafts; first written PSD contains the title at the original anchor/size/orientation; refused commit writes nothing; repeated saves share one completion; undo/redo and reopen preserve text |
| UI35 (four variants) | Horizontal/vertical draft → shortcut/toolbar Save → host storage refuses write → retry | Error is shown; original file bytes/version remain unchanged; committed text and dirty state survive; retry writes one title, then undo/redo/reopen preserve it |
| UI36 (three variants) | Delay first host write acknowledgement → repeat Save for same content → new draft or committed edit → request Save or leave unsaved | First PSD contains only original text; newer requested content receives one follow-up write; repeated requests coalesce; edits without a Save request stay dirty until explicitly saved |
| UI37 (four variants) | Hold a host write acknowledgement → request Undo/Redo → request Save before the history revision returns → release history/host in both orders | The follow-up write includes the requested history; first and final PSDs have the expected title count; duplicate saves coalesce; final dirty state clears and history/reopen still work |
| UI38 (two variants) | Submit horizontal/vertical text with another title selected → change font size while the Worker request is held → create another title | The older title stays unchanged; submitted text keeps its captured style; next text uses the new option; Undo/Redo and independently decoded PSD/reopen preserve all titles |
| UI39 (two variants) | Hold a real host write acknowledgement → queue Save for a new draft → cancel with Escape/the toolbar → save another title | Cancelled text never reaches the file or history; the queued save settles, later saves work and dirty state clears; decoded PSD and reopen retain only intended titles |
| UI40 (three variants) | Menus/Single line/Classic → Select All → disable/hide commands → Deselect → set 50% zoom and invoke View 100% → hide a menu | Common actions are directly clickable in Single line; selection and zoom change through real commands; host policy applies to engine and kit commands |
| UI41 (two variants) | Single line/Classic → disable/hide Image Adjustments → open the full group → Invert → Undo/Redo → save/reopen | Submenu policy holds; primary controls mount once; nonprimary commands remain clickable in overflowing popovers; exact red/cyan pixels and PSD persist correctly |
| UI42 (four variants) | Choose Brush from Home → keep/close the toolbar → host hides/disables all tools → click canvas → restore policy | Denied tools cannot paint or dirty the document without a rail fallback; restoring policy permits painting, Undo/Redo and PSD persistence |
| UI43 (four variants) | Begin a Brush/Rectangular Marquee drag → revoke tool permission → restore before/after release | Revocation cancels the pending gesture permanently; no pixels, selection or revision change; subsequent allowed drags work |
| UI44 (six variants) | Start Polygonal/Magnetic Lasso or Pen points with the toolbar closed → disable, or hide then restore → Enter | Revoked points never submit; recovery starts fresh geometry, supports Undo/Redo and preserves the Pen work path in PSD |
| UI45 (four variants) | Propose Crop with toolbar closed → hide/disable → Enter before/after restoration | Old proposal cannot crop; a new crop supports history and PSD persistence |
| UI46 (two variants) | Open Rasterize prompt → hide/disable Brush → restore | Old prompt closes without changing the Shape; new rasterization and painting remain separate undo steps |
| UI47 (eight variants) | Type/Vertical Type draft → revoke tool → Enter, shortcut Save, toolbar Save or blur → restore | No layer or file write while denied; text remains readable, cancellable and recoverable; final edit/history/PSD are correct |
| UI48 (two variants) | Hold rasterization → revoke Brush → release before/after restoration | Submitted rasterization can finish; old paint cannot replay; new paint and history work |
| UI49 (two variants) | Release Move while Auto-Select query waits → revoke → release query | No old translation or dirty change, including after restoration; new drag/history/PSD work |
| UI50 (two variants) | Hold rasterization → focus zoom/opacity → release | Completion preserves the later focus choice; field editing and Undo still work |
| UI51 (two variants) | Propose Crop/Rasterize → externally replace the same file | Old proposal disappears; replacement dimensions, Shape, pixels and clean state remain intact |
| UI52 (three variants) | Choose font twice in Options, Properties or across both → finish first query last | Latest font wins across controls; exactly one history step; PSD preserves the font |
| UI53 (two variants) | Hold Options/Properties font query → externally replace document | Disposed query cannot restyle replacement text; new font choice and PSD work |
| UI54 | Set Clone Stamp source → externally replace document → click → set new source and paint | Old source is cleared; new source, history and saved pixels work |
| UI55 | Target a layer mask → externally replace document with masked layer → paint | New document starts on pixels; deliberate mask selection still works; pixel/mask output persists in PSD |
| UI56 | Hold mask metadata → submit Brush → externally replace document | Waiting command cannot paint the replacement; clean state, subsequent drawing/history/PSD are correct |
| UI57 (two variants) | Sample foreground/background twice → complete first sample last | Latest sample wins in controls and engine colors without dirtying the document |
| UI58 (two variants) | Hold Eyedropper sample → revoke tool → complete before/after restoration | Old sample cannot change colors; a fresh allowed sample works |
| UI59 (three variants) | Show Navigator/Histogram/Info → externally replace with equal revision and different pixels | Cached image, histogram and RGB values refresh for the replacement without a document edit |
| UI60 (four variants) | Drag a layer → hide/disable the list → release before/after restoration | Revoked drag cannot reorder; fresh drag, Undo/Redo and PSD work |
| UI61 | Drag a layer → externally replace the document before release | Old drag cannot reorder replacement layers; a fresh drag and history/save remain usable |
| UI62 (four variants) | Choose a font for Type/Vertical Type while metadata waits → Enter/Save | Submitted text uses the chosen font; one-step history and PSD persistence remain correct |
| UI63 (two variants) | Submit draft while font metadata waits → revoke/restore permission or replace document | Pending creation aborts without losing draft text; explicit retry saves with the selected font |
| UI64 (two variants) | Choose an existing title's font through Options/Properties → immediately save with keyboard | Save waits for metadata and the resulting style command; file and clean state include the chosen font |
| GEST01 | Send 200,000 brush samples to the production tool → release | One command receives every point and the expected repaint bounds without a stack overflow |
| GEST02 (three variants) | Quick Selection, Lasso or Direct Selection → release at a newer position | Commands contain the final selection vertex or anchor displacement |
| GEST03 | Press/release Brush at the same position | Exactly one dab; endpoint handling does not duplicate click samples |
| GEST04 | Drag a Pen handle using only press/release → add second anchor → finish path | First anchor has the expected symmetric handles; second anchor remains a corner |
| GEST05 (six variants) | Scrubby Zoom release without move, after partial move, leftward, back to press, ordinary click or Alt-click | Final zoom and anchor match the gesture; clicks zoom exactly once |
| GEST06 | Rectangle/ellipse at fractional coordinates: click, horizontal/vertical drag, small nonempty drag, fixed ratio, fixed size and Shift constraint | Empty gestures deselect; nonempty or constrained gestures retain their selection behavior |
| GEST07 | Fixed-size rectangle/ellipse, 40×24 or 1×1, click and all four drag directions from fractional coordinates | Command dimensions stay exact and the anchor snaps consistently |
| GEST08 | Rectangle/ellipse × two fractional anchors × four directions × Shift, 1:1, 3:2, 2:3 and decimal 1.5:1 constraints (80 inputs) | Integer dimensions follow the constrained extent and do not grow with anchor fractions |
| GEST09 | Rectangle/ellipse/lasso × four toolbar modes × integer/fractional starts × ordinary/Shift/Alt/Shift+Alt clicks and two-point horizontal gestures (120 inputs) | Empty add/subtract submits no document command and clears the transient preview; replace/intersect deselect; press modifiers take precedence |
| GEST10 | Five shape tools, Crop, Slice and Object Selection × click, zero-width/height, return-to-press and valid release (40 inputs) | Empty commits clear the preview without commands/proposals; valid release uses its final bounds and retains the preview for commit |

UI08 injects browser keyboard events with composition flags to reproduce the handler boundary;
it does not automate the operating system's IME candidate window. UI05 retains its Chromium CDP
composition coverage. UI14/UI15/UI19 use a real mouse press followed by a controlled `pointerup` on
the canvas to reproduce sparse input. UI25 first draws a real preview, then delivers a controlled
`pointerup` or `pointercancel` and inspects the transient canvas before any repaint.
UI26/UI28 and the later async cases hold selected outbound requests to expose pending-command
boundaries, then release them to the real worker or let document disposal settle them. They do not
substitute successful engine results. UI51/UI53–56/UI59/UI61 change the host's bytes and file version
for the same path, exercising the file watcher without replacing the iframe or React app.
GEST01–10 import production tools in the browser and
capture commands or viewport calls before WASM; they verify input handling, not dense-stroke rendering performance
or selection algorithms. The suite contains 190 editor cases and 17 tool contract cases per browser.

## Reports and CI

The workflow is `.github/workflows/ci.yml`, which runs Kition's kit CI: every `test:*` script of
`web/` (flow, modes, focus, auto-select, parity, engine scenarios, UI scenarios) is its own job, on pushes to `ci/**`, pull requests, manual runs
and a nightly run on `main`. The workflow needs no secrets: it installs
the published web packages from npm (locked by `web/pnpm-lock.yaml`), Cargo
fetches the kition-sdk crates at the commit pinned in `Cargo.toml`, and `KitionAI/kition-fonts` is checked out at `main`. No `pull_request_target` execution is used.

The scenarios use zero retries, isolated documents, stable test IDs and condition polling.
Missing fonts/engines and uncaught page errors are failures. Static parity metadata is
not treated as behavioral proof. The broad flow also fails on uncaught page errors and
preserves its per-step results.

Artifacts under `web/` (gitignored):

- `test-results/build.json`: Node/platform and WASM/font SHA-256 identities.
- `test-results/coverage.json`: all 991 inventory entries, declared status and scenario command mapping; uncovered entries remain visible.
- `test-results/engine.xml`: engine JUnit report when using the CI entry point.
- `test-results/engine/<scenario>/`: failing operation, document state and actual PNG; checkpoint
  mismatches also retain the expected PNG and an amplified pixel difference SVG.
- `test-results/browser.xml`, `playwright-report/`: browser JUnit/HTML reports.
- `test-results/browser/`: failing screenshots, Playwright traces, document-state attachments.
- `test-results/legacy-<browser>/`: broad-flow JSON results and failure screenshot/trace.

A failed job uploads its `test-results/` and `playwright-report/` as the `evidence-<script>`
artifact.

## Coverage boundaries

The inventory has 991 entries (539 declared done, 407 todo, 45 hidden). These numbers
describe implementation declarations, not the number of verified behaviors. The 86
parity/unit tests and broad UI flow cover menu/tool catalogs, many options and host policies;
this suite adds independent user scenarios and result assertions.

Remaining expansion includes richer filter/adjustment combinations, CMYK/ICC conversion,
object-layer editing, large-image performance budgets, real host save-conflict/disk-write integration,
and persistent multi-document switching inside the full Kition application. UI07 verifies
independent host tabs, not the full application's tab manager. WebKit's text test is not an IME
composition test. The small generated PSDs do not replace the pinned real-file corpus checks.

Add a scenario to `cases.json` with a stable ID, explicit expected results and format-specific
tolerances only where justified. Add an isolated browser test for its UI workflow, update this
table and the command coverage report, and keep the normal test path free of reference-app access.

