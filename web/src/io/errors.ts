/**
 * The engine's errors as a person reads them (plan 2026-11-24, stage 6): the engine answers in
 * English with command ids (`command \`edit.paste\` is not available right now: the clipboard is
 * empty`); the editor shows a sentence of its own catalog instead (`error.<kind>`, en-US and
 * zh-CN in `strings.ts`), and only an error it does not know shows its text, folded under the
 * generic sentence. Pure, so `node --test` checks it.
 */

/** The kinds of error the editor words itself, each with what in the engine's text gives it away (first match wins). */
const KINDS: ReadonlyArray<readonly [string, RegExp]> = [
  ['cancelled', /^cancel+ed\b|\bwas cancel+ed\b/i],
  ['clipboardEmpty', /clipboard is empty|nothing (?:was )?copied/i],
  ['noDocument', /no (?:active )?document|no document (?:is )?open/i],
  ['busy', /background job is running|is busy/i],
  ['locked', /\blocked\b/i],
  ['noSelection', /no selection|selection is empty|needs a selection|without a selection/i],
  ['tooLarge', /megapixels|too large|too big|limited to|exceeds|out of memory|allocation|larger than/i],
  ['unreadableImage', /decod|not a supported|unknown (?:image )?format|unsupported (?:file|image)? ?format|unrecognized|not an image|could not read the image|invalid (?:png|jpeg|jpg|webp|gif|tiff|image)|is not valid base64|corrupt/i],
  ['noFileSystem', /no file system|not available on the web|needs the desktop app|not available in the browser|ambient filesystem/i],
  ['hidden', /\bhidden\b/i],
  ['notType', /not a type layer|no type layers|needs a type layer/i],
  ['notShape', /not a shape layer|no shape path|needs a shape/i],
  ['noMask', /has no (?:layer |vector )?mask/i],
  ['noPath', /no work path|no path named|needs a path/i],
  ['notPixels', /not a pixel layer|has no pixels|no pixels to|rasteriz|not an object layer/i],
  ['noLayer', /no (?:active )?layer|no layer selected|no such layer|layer \d+ (?:does not exist|not found)/i],
  ['nothingToDo', /nothing to (?:transform|purge|do)|can't be repeated|no layer has effects|keep at least one layer/i],
  ['badValue', /invalid parameters|must be|missing `|out of range|not a number|unknown (?:option|value)/i],
  ['notNow', /is not available right now|not available now|is disabled/i],
  ['denied', /permission|not allowed|denied|read-only|view only/i],
]

export type ExplainedError = {
  /** The catalog key of the sentence to show (`error.<kind>`, `error.generic` when unknown). */
  key: string
  /** The engine's own text, for an error the editor does not know; null when the sentence says it all. */
  detail: string | null
}

/** The text of `error` (an Error, a string, or a JSON-RPC style `{message}`). */
export function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'string') return error
  if (error && typeof error === 'object' && typeof (error as { message?: unknown }).message === 'string') return (error as { message: string }).message
  return String(error)
}

/** `text` without the engine's command ids and wrappers: what is left says what went wrong. */
export function withoutIds(text: string): string {
  return text
    .replace(/^(?:step \d+ \(`[^`]*`\) failed(?: on the draft)?:\s*)+/i, '')
    .replace(/command `[^`]*` is not available right now:\s*/gi, '')
    .replace(/invalid parameters for `[^`]*`:\s*/gi, '')
    .replace(/automation command `[^`]*`\s*/gi, '')
    .replace(/`[a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)+`:?\s*/g, '')
    .trim()
}

/** What to tell the person about `error`. */
export function explainError(error: unknown): ExplainedError {
  const text = errorText(error)
  const kind = KINDS.find(([, pattern]) => pattern.test(text))
  if (kind) return { key: `error.${kind[0]}`, detail: null }
  const detail = withoutIds(text)
  return { key: 'error.generic', detail: detail || null }
}

/** The kinds `explainError` words (each needs `error.<kind>` in both catalogs). */
export const ERROR_KINDS: readonly string[] = [...KINDS.map(([kind]) => kind), 'generic']
