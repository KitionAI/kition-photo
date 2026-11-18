import type { KitTool } from '@kition/kit-editor'
import type { PhotoToolEnv } from './common'

/** The Horizontal and Vertical Type tools: a click opens a field for the text there. */
export function typeTool(id: 'Type' | 'VerticalType', env: PhotoToolEnv): KitTool {
  const orientation = id === 'Type' ? 'horizontal' : 'vertical'
  return {
    id,
    cursor: id === 'Type' ? 'text' : 'vertical-text',
    begin: (pointer) => env.beginText(pointer, orientation),
  }
}
