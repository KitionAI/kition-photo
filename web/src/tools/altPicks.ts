import type { PhotoToolId } from './catalog'

/** The tools whose Alt-press picks the foreground colour instead (Photoshop's temporary Eyedropper, plan 2026-11-24, stage 2). */
export const ALT_PICKS: ReadonlySet<PhotoToolId> = new Set<PhotoToolId>(['Brush', 'Pencil', 'BackgroundEraser', 'PaintBucket', 'Gradient'])

/** Whether a press with these keys picks a colour rather than paints (Cmd and Alt together is no pick). */
export const picksColour = (id: PhotoToolId, pointer: { alt: boolean; meta?: boolean }) => ALT_PICKS.has(id) && pointer.alt && !pointer.meta
