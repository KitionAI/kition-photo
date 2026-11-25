// Builds photo-upstream.json from upstream's ui-egui source (menu_catalog.rs, menus.rs, state.rs,
// panels.rs, dock.rs) and the built web engine (`pnpm --dir web engine`; upstream's engine, whose
// commands upstream's menus also draw). Node, no dependencies: run `KITION_UPSTREAM_UI=<path to
// the upstream crates/ui-egui checkout> node web/parity/extract.mjs` whenever upstream or the
// engine's commands change; the output is committed, so this does not run in CI.
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath, pathToFileURL } from 'node:url'
import path from 'node:path'

const upstreamRoot = process.env.KITION_UPSTREAM_UI
if (!upstreamRoot) {
  console.error('KITION_UPSTREAM_UI is not set: point it at the upstream crates/ui-egui checkout (see docs/photo-ui-parity plan).')
  process.exit(1)
}

const srcDir = path.join(upstreamRoot, 'src')
const read = (name) => readFileSync(path.join(srcDir, name), 'utf8')

const menuCatalogText = read('menu_catalog.rs')
const menusText = read('menus.rs')
const stateText = read('state.rs')
const panelsText = read('panels.rs')
const dockText = read('dock.rs')

// ---------------------------------------------------------------------------------- helpers

/** Unescapes the limited set of escapes the upstream source uses inside its string literals. */
function unescapeRustString(s) {
  return s.replace(/\\(.)/g, (_, c) => (c === 'n' ? '\n' : c === 't' ? '\t' : c))
}

/**
 * Finds the balanced bracket group whose opening bracket is the last character matched by
 * `anchor` (a regex that must end exactly on `{`, `[`, or `(`), then returns its contents.
 * Skips string literals, char literals, and line comments while counting depth, so brace-like
 * characters inside an i18n template string (`"{key}-click"`) or a `'\0'` char literal never
 * throw off the count.
 */
function balancedAfter(text, anchor) {
  const m = anchor.exec(text)
  if (!m) throw new Error(`anchor not found: ${anchor}`)
  const open = m.index + m[0].length - 1
  if (!'{[('.includes(text[open])) throw new Error(`anchor ${anchor} did not end on an opening bracket (got ${JSON.stringify(text[open])})`)
  return balancedAt(text, open)
}

/** The contents of the bracket group that opens at `open` (see `balancedAfter`). */
function balancedAt(text, open) {
  let depth = 0
  let i = open
  while (i < text.length) {
    const c = text[i]
    if (c === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') i++
      continue
    }
    if (c === '"') {
      i++
      while (i < text.length && text[i] !== '"') {
        if (text[i] === '\\') i++
        i++
      }
      i++
      continue
    }
    if (c === "'") {
      // A char literal ('V', '\0', '\\') closes within two or three characters; anything else
      // (a lifetime like `&'static`) is not, so only consume it as a literal when it does.
      let j = i + 1
      if (text[j] === '\\') j++
      j++
      if (text[j] === "'") {
        i = j + 1
        continue
      }
      i++
      continue
    }
    if (c === '{' || c === '(' || c === '[') depth++
    else if (c === '}' || c === ')' || c === ']') {
      depth--
      if (depth === 0) return text.slice(open + 1, i)
    }
    i++
  }
  throw new Error(`unbalanced block starting at ${open}`)
}

// ---------------------------------------------------------------------------------- menus

/**
 * `menu_catalog.rs`'s `CATALOG` is a flat list of
 * `(&["Path", …], "Label", Some("Shortcut") | None, "command.id")` tuples, one static array.
 * Entries are one-liners in practice, but the regex tolerates wrapping (dotall) and escaped
 * quotes inside labels and paths, since upstream text is not under our control.
 */
function parseMenus(text) {
  const body = balancedAfter(text, /pub static CATALOG[\s\S]*?=\s*&\[/)
  const tupleRe = /\(\s*&\[(?<path>(?:"(?:[^"\\]|\\.)*"\s*,?\s*)*)\]\s*,\s*"(?<label>(?:[^"\\]|\\.)*)"\s*,\s*(?:Some\(\s*"(?<shortcut>(?:[^"\\]|\\.)*)"\s*\)|None)\s*,\s*"(?<command>(?:[^"\\]|\\.)*)"\s*\)/gs
  const menus = []
  let m
  while ((m = tupleRe.exec(body))) {
    const pathStrings = [...m.groups.path.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((p) => unescapeRustString(p[1]))
    const label = unescapeRustString(m.groups.label)
    const shortcut = m.groups.shortcut != null ? unescapeRustString(m.groups.shortcut) : null
    const command = unescapeRustString(m.groups.command)
    menus.push({ path: pathStrings, label, shortcut, command })
  }
  return menus
}

/** `menus.rs`'s `PLACE_AFTER`: engine commands that belong right after a catalog row, `[id, after]`. */
function parsePlaceAfter(text) {
  const body = balancedAfter(text, /const PLACE_AFTER: &\[\(&str, &str\)\] =\s*&\[/)
  return [...body.matchAll(/\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g)].map((m) => [unescapeRustString(m[1]), unescapeRustString(m[2])])
}

/**
 * `menus.rs`'s `menu_items` step 2 for the engine's commands: each command with a menu path that
 * the catalog lacks (by id, or by path and label without a trailing "…") goes right after its
 * `PLACE_AFTER` row, else after the last row of its top-level menu, in the engine's order.
 * Upstream's `UI_COMMANDS` (its own shell's rows: Help links, themes,
 * window toggles) are left out; Kition draws its own. The manifest keeps upstream's ids: an engine
 * command is matched and written under the id upstream has for it (`photo-upstream-ids.json`,
 * upstream id → the engine's id).
 */
function withEngineRows(catalog, engineCommands, placeAfter) {
  const upstreamOf = new Map(Object.entries(JSON.parse(readFileSync(new URL('./photo-upstream-ids.json', import.meta.url), 'utf8'))).map(([upstream, engine]) => [engine, upstream]))
  const commands = engineCommands.map((command) => ({ ...command, id: upstreamOf.get(command.id) ?? command.id }))
  const items = catalog.slice()
  const bare = (label) => label.replace(/…+$/, '')
  for (const command of commands.filter((item) => item.menu?.length)) {
    const where = command.menu.join('/')
    if (items.some((item) => item.command === command.id || (item.path.join('/') === where && bare(item.label) === bare(command.title)))) continue
    const target = placeAfter.find(([id]) => id === command.id)
    const after = target ? items.findIndex((item) => item.command === target[1]) : -1
    const at = after >= 0 ? after : items.findLastIndex((item) => item.path[0] === command.menu[0])
    items.splice(at < 0 ? items.length : at + 1, 0, { path: [...command.menu], label: command.title, shortcut: command.shortcut ?? null, command: command.id })
  }
  return items
}

/** The built web engine's commands, in its registry's order. */
async function engineCommands() {
  const dir = fileURLToPath(new URL('../public/engine/', import.meta.url))
  const wasm = path.join(dir, 'kition_photo_kit_bg.wasm')
  if (!existsSync(wasm)) {
    console.error(`no built engine at ${dir}: run \`pnpm --dir web engine\` first.`)
    process.exit(1)
  }
  const sdk = await import(pathToFileURL(path.join(dir, 'kition_photo_kit.js')).href)
  sdk.initSync({ module: readFileSync(wasm) })
  return JSON.parse(sdk.PhotoDocument.create('Untitled', 16, 16).commands())
}

// ---------------------------------------------------------------------------------- tools

/** `state.rs`'s `Tool::label(self)`: one `Tool::Name => "Label",` arm per tool, no combined arms. */
function parseToolLabels(text) {
  const body = balancedAfter(text, /pub fn label\(self\) -> &'static str\s*\{/)
  const labels = new Map()
  const re = /Tool::(\w+)\s*=>\s*"((?:[^"\\]|\\.)*)"/g
  let m
  while ((m = re.exec(body))) labels.set(m[1], unescapeRustString(m[2]))
  return labels
}

/**
 * `state.rs`'s `Tool::key(self)`: arms can cover several tools at once
 * (`Tool::RectMarquee | Tool::EllipseMarquee => 'M',`) and `'\0'` means no shortcut key.
 */
function parseToolKeys(text) {
  const body = balancedAfter(text, /pub fn key\(self\) -> char\s*\{/)
  const keys = new Map()
  const re = /((?:Tool::\w+)(?:\s*\|\s*Tool::\w+)*)\s*=>\s*'((?:[^'\\]|\\.)*)'/g
  let m
  while ((m = re.exec(body))) {
    const tools = [...m[1].matchAll(/Tool::(\w+)/g)].map((t) => t[1])
    const raw = m[2]
    const key = raw === '\\0' ? null : unescapeRustString(raw)
    for (const tool of tools) keys.set(tool, key)
  }
  return keys
}

/**
 * Parses a Rust array literal of (possibly nested) arrays whose only leaves are `Tool::Name`
 * identifiers, returning nested JS arrays of bare names. Handles arbitrary depth so it keeps
 * working if upstream adds another nesting level.
 */
function parseToolArrayLiteral(body) {
  function parseFrom(i) {
    while (body[i] !== '[') i++
    i++
    const items = []
    while (true) {
      while (/\s/.test(body[i]) || body[i] === ',') i++
      if (body[i] === ']') return { items, next: i + 1 }
      if (body[i] === '&') {
        i++
        continue
      }
      if (body[i] === '[') {
        const { items: nested, next } = parseFrom(i)
        items.push(nested)
        i = next
        continue
      }
      const m = /^Tool::(\w+)/.exec(body.slice(i, i + 64))
      if (!m) throw new Error(`unexpected token at ${i}: ${body.slice(i, i + 20)}`)
      items.push(m[1])
      i += m[0].length
    }
  }
  return parseFrom(0).items
}

/** `panels.rs`'s `TOOL_SECTIONS`: sections of slots of flyout tools, Photoshop toolbar order. */
function parseToolSections(text) {
  const body = balancedAfter(text, /const TOOL_SECTIONS: &\[&\[&\[Tool\]\]\] =\s*&\[/)
  return parseToolArrayLiteral(`[${body}]`)
}

function parseTools(stateSrc, panelsSrc) {
  const labels = parseToolLabels(stateSrc)
  const keys = parseToolKeys(stateSrc)
  const sections = parseToolSections(panelsSrc)
  const tools = []
  sections.forEach((section, sectionIndex) => {
    section.forEach((slot, slotIndex) => {
      for (const id of slot) {
        tools.push({ id, label: labels.get(id) ?? null, key: keys.get(id) ?? null, section: sectionIndex, slot: slotIndex })
      }
    })
  })
  return { tools, sectionCount: sections.length }
}

// ---------------------------------------------------------------------------------- dock

/**
 * `dock.rs`'s `Group::tabs(self, pro)`. The Essentials ("pro") workspace order is canonical
 * (it is also the order documented on `Group`'s doc comments and used by `Group::ALL`); the
 * one non-pro arm (Color) only reorders the same tab set and is not tracked separately here.
 */
function parseDockGroups(text) {
  const body = balancedAfter(text, /pub fn tabs\(self, pro: bool\) -> &'static \[&'static str\]\s*\{/)
  const re = /Group::(\w+)(\s+if\s+pro)?\s*=>\s*&\[((?:"(?:[^"\\]|\\.)*"\s*,?\s*)*)\]/g
  const proTabs = new Map()
  const plainTabs = new Map()
  let m
  while ((m = re.exec(body))) {
    const group = m[1]
    const tabs = [...m[3].matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((t) => unescapeRustString(t[1]))
    if (m[2]) proTabs.set(group, tabs)
    else plainTabs.set(group, tabs)
  }
  // `Group::ALL`'s declared order (dock.rs) is Photoshop's Essentials order, top to bottom.
  const orderMatch = /pub const ALL: \[Group; \d+\] = \[([^\]]*)\]/.exec(text)
  const order = [...orderMatch[1].matchAll(/Group::(\w+)/g)].map((g) => g[1])
  const keyBody = balancedAfter(text, /pub fn key\(self\) -> &'static str\s*\{/)
  const keyRe = /Group::(\w+)\s*=>\s*"((?:[^"\\]|\\.)*)"/g
  const keys = new Map()
  let km
  while ((km = keyRe.exec(keyBody))) keys.set(km[1], km[2])
  /** "Layer Comps" -> "layerComps", matching the `window.panel.layerComps`-style ids already
   * used for the same panels in `menu_catalog.rs`. */
  const toTabId = (label) =>
    label
      .split(/\s+/)
      .map((w, i) => (i === 0 ? w[0].toLowerCase() + w.slice(1) : w[0].toUpperCase() + w.slice(1)))
      .join('')
  return order.map((group) => {
    const tabs = (proTabs.get(group) ?? plainTabs.get(group) ?? []).map(toTabId)
    return { group: keys.get(group), tabs }
  })
}

// ---------------------------------------------------------------------------------- options

const eraserText = read('eraser_ui.rs')
const retouchText = read('retouch_ui.rs')
const vectorText = read('vector_ui.rs')
const analysisText = read('analysis_ui.rs')
const sliceText = read('slice_ui.rs')
const typeText = read('type_tool.rs')

/** Skips a string or char literal at `i`; the index after it, or -1 when there is none. */
function skipLiteral(s, i) {
  if (s[i] === '"') {
    i++
    while (i < s.length && s[i] !== '"') {
      if (s[i] === '\\') i++
      i++
    }
    return i + 1
  }
  if (s[i] === "'") {
    let j = i + 1
    if (s[j] === '\\') j++
    j++
    if (s[j] === "'") return j + 1
  }
  return -1
}

/** A `match` body's arms, `Tool::X [| Tool::Y]* [if …] => …`, split at bracket depth 0: their tools and text. */
function matchArms(body) {
  const armHeaderRe = /^Tool::\w+(?:\s*\|\s*Tool::\w+)*\s*(?:if\s+[^{=]*)?=>/
  const boundaries = []
  let depth = 0
  let i = 0
  while (i < body.length) {
    const c = body[i]
    if (c === '/' && body[i + 1] === '/') {
      while (i < body.length && body[i] !== '\n') i++
      continue
    }
    const skipped = skipLiteral(body, i)
    if (skipped !== -1) {
      i = skipped
      continue
    }
    if (depth === 0) {
      const hm = armHeaderRe.exec(body.slice(i))
      if (hm) {
        boundaries.push(i)
        i += hm[0].length
        continue
      }
    }
    if (c === '{' || c === '(' || c === '[') depth++
    else if (c === '}' || c === ')' || c === ']') depth--
    i++
  }
  return boundaries.map((start, idx) => {
    const text = body.slice(start, boundaries[idx + 1] ?? body.length)
    return { tools: [...armHeaderRe.exec(text)[0].matchAll(/Tool::(\w+)/g)].map((t) => t[1]), text }
  })
}

const TL = String.raw`tl!\("((?:[^"\\]|\\.)*)"\)`

/** Whether a free label (one drawn by a label call, not a control) names a field rather than giving a hint. */
const isFieldLabel = (label) => !label.includes('·') && (label.trim().endsWith(':') || label.trim().split(/\s+/).length <= 3)

/**
 * The fields of an options-bar fragment, in source order: labels (`opt_label`, `percent_field`,
 * `pct`, and `opt`/`lbl`/`RichText`/`readout` labels that name a field rather than hint), check
 * boxes and toggles, secondary (text) buttons, icon buttons (their tooltip), the `tl!` entries of
 * a `for … in [ … ]` list (Mixer's fields, segmented icon buttons, align buttons), unlabeled
 * drop-downs as `dropdown:<id>`, and upstream's shared bar helpers.
 */
function fieldsIn(text) {
  // `label` hits name the control after them (a drop-down, a text field, a colour button).
  const hits = []
  const add = (index, label, kind = 'control') => hits.push({ index, label: unescapeRustString(label), kind })
  const patterns = [
    [new RegExp(String.raw`opt_label\(ui,\s*${TL}`, 'g'), 'label'],
    [new RegExp(String.raw`percent_field\(ui,\s*${TL}`, 'g'), 'control'],
    [new RegExp(String.raw`\bpct\(ui,\s*${TL}`, 'g'), 'control'],
    [new RegExp(String.raw`\b(?:opt|lbl)\(ui,\s*${TL}\)`, 'g'), 'free'],
    [new RegExp(String.raw`RichText::new\(${TL}\)`, 'g'), 'free'],
    [/readout\(ui,\s*"((?:[^"\\]|\\.)*)"/g, 'control'],
    [new RegExp(String.raw`widgets::(?:checkbox|toggle)\(ui,\s*&mut\s*[^,]+,\s*${TL}`, 'g'), 'control'],
    [new RegExp(String.raw`secondary_button\(ui,\s*(?:${TL}|"((?:[^"\\]|\\.)*)")`, 'g'), 'control'],
    [new RegExp(String.raw`icons::button\(\s*ui,\s*(?:"[^"]*"|\w+),\s*[\d.]+,\s*[^,]+,\s*(?:&crate::i18n::fmt\()?${TL}`, 'g'), 'control'],
  ]
  for (const [re, kind] of patterns)
    for (const m of text.matchAll(re)) {
      const label = m[1] ?? m[2]
      if (kind === 'free' && !isFieldLabel(label)) continue
      add(m.index, label, kind === 'control' ? 'control' : 'label')
    }
  for (const m of text.matchAll(/widgets::dropdown\(\s*ui,\s*"([\w-]+)"/g)) add(m.index, `dropdown:${m[1]}`)
  for (const m of text.matchAll(/\bfor\b[^{;]*?\bin\s*\[/g)) {
    const list = balancedAt(text, m.index + m[0].length - 1)
    for (const t of list.matchAll(new RegExp(TL, 'g'))) add(m.index + t.index, t[1])
  }
  const helpers = [
    [/selection_mode_buttons\(app, ui\)/g, () => selectionModes()],
    [/brush_preset_chip\(ui/g, () => ['Brush Preset picker']],
    [/brush_picker::settings_toggle\(/g, () => ['Toggle the Brush Settings panel']],
    [/font_picker\(ui/g, () => ['dropdown:type-font']],
    [/preset_panels::shape_picker\(/g, () => ['Shape:']],
    [/gradient_swatch\(ui|gradient_ui::preset_swatch\(/g, () => ['Click to edit the gradient']],
    [/color_edit_button_rgb\(/g, () => ['color']],
    [/swatch_popup\(/g, () => ['Set the text color']],
    [/TextEdit::singleline\(/g, () => ['text']],
    // Unlabeled number fields: the Crop ratio's width and height, the Type size (after its icon).
    [/value_field\(ui, &mut rw\b/g, () => ['Width']],
    [/value_field\(ui, &mut rh\b/g, () => ['Height']],
    [/value_field\(ui, &mut size, 1\.0\.\.=1296/g, () => ['Size']],
  ]
  for (const [re, labels] of helpers) for (const m of text.matchAll(re)) labels().forEach((label, k) => hits.push({ index: m.index + k / 100, label, kind: 'helper' }))
  hits.sort((a, b) => a.index - b.index)
  // A drop-down, text field, or colour button right after its label is that label's field.
  const fields = hits.filter((hit, at) => !(at > 0 && hits[at - 1].kind === 'label' && (hit.label.startsWith('dropdown:') || ['text', 'color'].includes(hit.label))))
  return [...new Set(fields.map((h) => h.label))]
}

let selectionModeFields = null
/** The New, Add, Subtract, and Intersect buttons of `selection_mode_buttons`, which several bars call. */
function selectionModes() {
  selectionModeFields ??= fieldsIn(balancedAfter(panelsText, /fn selection_mode_buttons\([^)]*\)\s*\{/))
  return selectionModeFields
}

/**
 * Splits a fragment's fields by the tool they show for: inside `if [app.ui.]tool == Tool::X {…}`
 * a field is only Tool::X's (Width, Contrast, and Frequency are the Magnetic Lasso's alone).
 */
function fieldsByTool(text, tools) {
  const only = []
  for (const m of text.matchAll(/if\s+(?:app\.ui\.)?tool\s*==\s*Tool::(\w+)\s*\{/g)) {
    const open = m.index + m[0].length - 1
    const inner = balancedAt(text, open)
    only.push({ tool: m[1], start: open, end: open + inner.length + 2, fields: fieldsIn(inner) })
  }
  const outside = only.reduceRight((rest, block) => rest.slice(0, block.start) + ' '.repeat(block.end - block.start) + rest.slice(block.end), text)
  const common = fieldsIn(outside)
  return tools.map((tool) => ({ tools: [tool], fields: [...new Set([...common, ...only.filter((block) => block.tool === tool).flatMap((block) => block.fields)])] }))
}

/** Groups with the same fields merged back into one entry per field list. */
function mergeGroups(groups) {
  const byFields = new Map()
  for (const group of groups) {
    if (!group.fields.length) continue
    const key = JSON.stringify(group.fields)
    const known = byFields.get(key)
    if (known) known.tools.push(...group.tools.filter((tool) => !known.tools.includes(tool)))
    else byFields.set(key, { tools: [...group.tools], fields: group.fields })
  }
  return [...byFields.values()]
}

/** The `options_bar` of a sub-module: its `match tool { … }` arms. */
function subModuleArms(text) {
  const fn = balancedAfter(text, /pub fn options_bar\(app: &mut \w+App, ui: &mut egui::Ui, tool: Tool\)[^{]*\{/)
  return matchArms(balancedAfter(fn, /match tool\s*\{/))
}

const SHAPE_TOOLS = ['Rectangle', 'EllipseShape', 'Triangle', 'Polygon', 'Line', 'CustomShape']
const BRUSH_CHIP_EXCEPT = ['Brush', 'Pencil', 'MixerBrush', 'Eraser']

/**
 * Upstream's options bars: `panels.rs`'s `options_bar` (its `match app.ui.tool` arms, and the
 * brush chip it draws before the retouching tools' and Quick Selection's bars), and the bars it
 * hands to `eraser_ui`, `retouch_ui`, `vector_ui`, `analysis_ui`, `slice_ui`, and `type_tool`.
 * The Gradient's live-layer options (`gradient_ui::options_changed`) change a selected gradient
 * fill layer through these same fields and draw none of their own.
 */
function parseOptions() {
  const groups = []
  const panelsBar = balancedAfter(panelsText, /pub fn options_bar\(app: &mut \w+App, ui: &mut egui::Ui\)[\s\S]*?\{/)
  for (const arm of matchArms(balancedAfter(panelsBar, /match app\.ui\.tool\s*\{/))) groups.push(...fieldsByTool(arm.text, arm.tools))
  for (const arm of [...subModuleArms(eraserText), ...subModuleArms(retouchText), ...subModuleArms(analysisText)]) groups.push(...fieldsByTool(arm.text, arm.tools))
  // The retouching tools and Quick Selection: the brush chip first (`is_brushlike` but not the four with their own chip).
  const chip = ['Brush Preset picker', 'Toggle the Brush Settings panel']
  const retouching = [...new Set(subModuleArms(retouchText).flatMap((arm) => arm.tools))].filter((tool) => !['Patch', 'ContentAwareMove', 'ObjectSelection'].includes(tool) && !BRUSH_CHIP_EXCEPT.includes(tool))
  groups.push({ tools: [...retouching, 'BackgroundEraser'], fields: chip })
  // vector_ui: Path Selection and Direct Selection draw hints only; the Pen its mode; the shapes their shared fields and each its own.
  const vectorBar = balancedAfter(vectorText, /pub fn options_bar\(app: &mut \w+App, ui: &mut egui::Ui, tool: Tool\)[^{]*\{/)
  groups.push({ tools: ['Pen'], fields: fieldsIn(balancedAfter(vectorBar, /if tool == Tool::Pen\s*\{/)) })
  const shapeCommon = vectorBar.slice(vectorBar.indexOf('let mut mode = "shape"'), vectorBar.indexOf('match tool {'))
  groups.push({ tools: SHAPE_TOOLS, fields: fieldsIn(shapeCommon) })
  for (const arm of matchArms(balancedAfter(vectorBar, /match tool\s*\{/))) groups.push({ tools: arm.tools, fields: fieldsIn(arm.text) })
  // slice_ui: the Slice tool, else Slice Select.
  const sliceBar = balancedAfter(sliceText, /pub fn options_bar\(app: &mut \w+App, ui: &mut egui::Ui, tool: Tool\)[^{]*\{/)
  groups.push({ tools: ['Slice'], fields: fieldsIn(balancedAfter(sliceBar, /if tool == Tool::Slice\s*\{/)) })
  groups.push({ tools: ['SliceSelect'], fields: fieldsIn(balancedAfter(sliceBar, /\}\s*else\s*\{/)) })
  // type_tool: the Type tools' bar.
  groups.push({ tools: ['Type', 'VerticalType'], fields: fieldsIn(balancedAfter(typeText, /pub fn options_bar\(app: &mut \w+App, ui: &mut egui::Ui\)\s*\{/)) })
  return mergeGroups(groups)
}

// ---------------------------------------------------------------------------------- reword

/**
 * Kition never uses another company's product names as its feature names: upstream's labels and
 * ids that are Photoshop's names are reworded to Kition Photo's generic ones, the same the engine
 * uses, before the engine's rows are merged in. A menu
 * row or tool says so (`reworded`; `idReworded` when its command or id changed too). Options
 * fields and dock tabs are reworded in place.
 */
const TRADEMARK_LABELS = [
  ['Content-Aware Move', 'Smart Move'],
  ['Content-Aware Fill', 'Smart Fill'],
  ['Content-Aware Scale', 'Smart Scale'],
  ['Content-Aware', 'Smart Fill'],
  ['Camera Raw', 'Raw Develop'],
  ['camera raw', 'raw develop'],
  ['Smart Object Layer', 'Object Layer'],
  ['Smart Objects', 'Object Layers'],
  ['Smart Object', 'Object Layer'],
  ['smart objects', 'object layers'],
  ['smart object', 'object layer'],
  ['Smart Filters', 'Editable Filters'],
  ['Smart Filter', 'Editable Filter'],
  ['Select and Mask', 'Refine Selection'],
  ['select and mask', 'refine selection'],
  ['Refine Edge', 'Edge Refine'],
  ['Puppet Warp', 'Pin Warp'],
  ['Puppet Pin', 'Warp Pin'],
]
const TRADEMARK_IDS = [
  ['ContentAwareMove', 'SmartMove'],
  ['contentAwareMove', 'smartMove'],
  ['contentAwareFill', 'smartFill'],
  ['contentAwareScale', 'smartScale'],
  ['contentAware', 'smart'],
  ['CameraRaw', 'RawDevelop'],
  ['cameraRaw', 'rawDevelop'],
  ['SmartObject', 'ObjectLayer'],
  ['smartObject', 'objectLayer'],
  ['SmartFilter', 'EditableFilter'],
  ['smartFilter', 'editableFilter'],
  ['SelectAndMask', 'RefineSelection'],
  ['selectAndMask', 'refineSelection'],
  ['refineEdge', 'edgeRefine'],
  ['PuppetWarp', 'PinWarp'],
  ['puppetWarp', 'pinWarp'],
]
const replaceAllOf = (pairs) => (s) => pairs.reduce((text, [from, to]) => text.replaceAll(from, to), s)
const rewordLabel = replaceAllOf(TRADEMARK_LABELS)
const rewordId = replaceAllOf(TRADEMARK_IDS)

/** A catalog row under Kition Photo's names. */
function rewordedRow(row) {
  const next = { ...row, path: row.path.map(rewordLabel), label: rewordLabel(row.label), command: rewordId(row.command) }
  const idReworded = next.command !== row.command
  if (!idReworded && next.label === row.label && next.path.join('/') === row.path.join('/')) return row
  return { ...next, reworded: true, ...(idReworded ? { idReworded } : {}) }
}

/** A tool under Kition Photo's names. */
function rewordedTool(tool) {
  const next = { ...tool, id: rewordId(tool.id), label: tool.label == null ? null : rewordLabel(tool.label) }
  const idReworded = next.id !== tool.id
  if (!idReworded && next.label === tool.label) return tool
  return { ...next, reworded: true, ...(idReworded ? { idReworded } : {}) }
}

// ---------------------------------------------------------------------------------- build

const catalog = parseMenus(menuCatalogText).map(rewordedRow)
const placeAfter = parsePlaceAfter(menusText).map((pair) => pair.map(rewordId))
const menus = withEngineRows(catalog, await engineCommands(), placeAfter)
const parsedTools = parseTools(stateText, panelsText)
const tools = parsedTools.tools.map(rewordedTool)
const { sectionCount } = parsedTools
const dock = parseDockGroups(dockText).map((group) => ({ ...group, tabs: group.tabs.map(rewordId) }))
const options = parseOptions().map((group) => ({ tools: group.tools.map(rewordId), fields: group.fields.map(rewordLabel) }))

const manifest = {
  source: { repository: 'KitionAI/kition-photo-archive', tag: 'v0.5.0', path: 'crates/ui-egui' },
  menus,
  tools,
  dock,
  options,
}

const outPath = fileURLToPath(new URL('./photo-upstream.json', import.meta.url))
writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`)

const nonSeparatorMenus = menus.filter((m) => m.command !== '---').length
console.log(`menus: ${menus.length} rows (${nonSeparatorMenus} non-separator, ${menus.length - nonSeparatorMenus} separators; ${menus.length - catalog.length} from the engine)`)
console.log(`tools: ${tools.length} in ${sectionCount} sections`)
console.log(`dock: ${dock.length} groups, ${dock.reduce((n, g) => n + g.tabs.length, 0)} tabs`)
console.log(`options: ${options.length} groups, ${options.reduce((n, g) => n + g.fields.length, 0)} fields`)
console.log(`reworded: ${[...menus, ...tools].filter((item) => item.reworded).length}`)
console.log(`wrote ${outPath}`)
