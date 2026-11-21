import { useCallback } from 'react'
import { useKitStore } from '@kition/sdk/editor'
import type { PhotoToolId } from '../tools/catalog'
import { defaultOptions, mergeOptions, withOptions, type ToolOptions } from '../tools/options'

/** How long the options rest before they are written (a scrubbed field changes them on every step). */
const WRITE_DELAY_MS = 400

/**
 * The tools' options (shared as upstream shares them), kept in Kit UI's kit store
 * (`kit:kit.photo:toolOptions`): read once (merged with upstream's defaults, whatever shape older
 * data has), written once they rest and when the page goes away, and followed when another Photo
 * tab changes them.
 */
export function useToolOptions() {
  const [state, store] = useKitStore('kit.photo', 'toolOptions', { initial: defaultOptions(), validate: mergeOptions, delayMs: WRITE_DELAY_MS })
  const set = useCallback((tool: PhotoToolId, patch: Partial<ToolOptions>) => void store((current) => withOptions(current, tool, patch)), [store])
  return [state, set] as const
}
