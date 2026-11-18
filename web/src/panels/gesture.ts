import { useMemo, useRef } from 'react'

/** A gesture's own coalescing key: each drag of a field is one history step, whatever came before. */
export const gestureKey = (what: string) => `${what}:${Date.now()}:${Math.random().toString(36).slice(2)}`

/**
 * Coalescing keys for a panel's fields, one per gesture: a field's drag (or a colour picker's
 * session) shares one key until its final value (or `end`), so the engine keeps it as one history
 * step; a typed value is a gesture of its own.
 */
export function useGestures(prefix: string) {
  const keys = useRef(new Map<string, string>())
  return useMemo(
    () => ({
      key(field: string, final: boolean) {
        const key = keys.current.get(field) ?? gestureKey(`${prefix}-${field}`)
        if (final) keys.current.delete(field)
        else keys.current.set(field, key)
        return key
      },
      end(field: string) {
        keys.current.delete(field)
      },
    }),
    [prefix],
  )
}
