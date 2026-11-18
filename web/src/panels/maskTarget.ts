import type { PhotoInfo } from '../photo'

/**
 * The mask target (upstream's `mask_target`, `with_mask_target`): after a click on a layer mask's
 * thumbnail, painting, fills, pastes, filters, and adjustments edit the active layer's mask
 * instead of its pixels, whichever way they run (canvas tools, menus, keys, dialogs). The engine
 * routes a command to the mask when its params say `"target": "mask"` and names the commands that
 * follow a target (its `maskTargetCommands` extra); the editor adds the target to those.
 */

/** Whether the mask target applies: the active layer has a mask, and the Channels panel targets the composite outside Quick Mask (an alpha channel or the Quick Mask keeps its own target). */
export function maskTargetApplies(on: boolean, info: PhotoInfo | null) {
  const active = info?.layers.find((layer) => layer.id === info.activeLayer)
  return on && Boolean(active?.hasMask) && (info?.channels?.target.kind ?? 'composite') === 'composite' && !info?.channels?.quickMask
}

/** `params` aimed at the active layer's mask while it is the target; unchanged otherwise, for a command that does not follow the target, or when they name a target. */
export function withMaskTarget(command: string, params: Record<string, unknown>, follows: ReadonlySet<string>, targeted: boolean): Record<string, unknown> {
  return targeted && params.target === undefined && follows.has(command) ? { ...params, target: 'mask' } : params
}

/** The engine's answer: the commands that follow the target, and those that cannot run aimed at the mask now. */
export type MaskTargetCommands = { follows: string[]; disabled: string[] }

/** The disabled commands while the mask is the target: the engine's own, but for the commands that follow the target, which run aimed at the mask. */
export function disabledWithMaskTarget(disabled: readonly string[], aimed: MaskTargetCommands): string[] {
  const follows = new Set(aimed.follows)
  return [...disabled.filter((command) => !follows.has(command)), ...aimed.disabled]
}
