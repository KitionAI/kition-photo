import type { KitTool } from '@kition/sdk/editor'
import type { PhotoToolEnv } from './common'

/**
 * The Horizontal and Vertical Type tools (plan 2026-11-24, stage 4): a press on type edits it
 * (the caret at the character there, a drag selecting); elsewhere a click places point text and a
 * drag a paragraph box. While text is edited, a press on it moves the caret or selects, a press on
 * a paragraph box's handle resizes the box, and a press elsewhere commits the edit (the editor's
 * `typePress`).
 */
export function typeTool(id: 'Type' | 'VerticalType', env: PhotoToolEnv): KitTool {
  const orientation = id === 'Type' ? 'horizontal' : 'vertical'
  return {
    id,
    cursor: id === 'Type' ? 'text' : 'vertical-text',
    begin: (pointer, context) => env.typePress(pointer, orientation, context),
    hover: (pointer, context) => env.typeHover?.(pointer, context),
    leave: () => env.setCursor(null),
  }
}
