import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { toast, useHostPick, useKitEditor, useUi, type KitCanvasHandle } from '@kition/sdk/editor'
import { ExportDialog, type Exported } from '../dialogs/ExportDialog'
import { exportPath, type ExportFormat } from './exportOptions'
import { OPEN_ACCEPT, PLACE_ACCEPT, dragHasFiles, dropCenters, fromBase64, isPlaceable, nameFor, toBase64 } from './files'
import { pushNotice, reportError } from './ErrorNotices'
import type { Busy } from './busy'

/**
 * Files in and out (plan 2026-11-24, stage 6), every one through the host or the browser's own
 * clipboard and drag: Export As and Quick Export (written beside the document by `host/write`),
 * Open (the host's picker and `host/open`), Place Embedded (the host's picker), Save As (the
 * host's service), the system clipboard (Cmd+C and Cmd+Shift+C put the copy there as a PNG as well
 * as in the engine's clipboard; Cmd+V pastes an image from it as a new layer), and image files
 * dragged from the computer onto the canvas or the Layers panel, placed where they drop. It draws
 * the Export dialog, the drop targets' marks and the paste catcher; App reaches it through `api`.
 */

export type FileIo = {
  exportAs: () => void
  quickExport: () => Promise<void>
  open: () => Promise<void>
  place: () => Promise<void>
  saveAs: () => Promise<void>
  /** Edit › Copy or Copy Merged, also onto the system clipboard. */
  copy: (command: 'edit.copy' | 'edit.copyMerged') => void
  /** Edit › Paste from the menu: the system clipboard's image when the browser lets the editor read it, else the engine's. */
  paste: () => void
}

/** What the host lets the File menu do (protocol capabilities, services and the document's permissions). */
export function fileAbilities(host: { init: { capabilities: readonly string[] }; hasService: (service: 'file.saveAs') => boolean; permissions: { export: boolean }; readOnly?: boolean }) {
  const caps = host.init.capabilities
  return {
    open: caps.includes('open-file') && caps.includes('pick-file'),
    place: caps.includes('pick-file') && !host.readOnly,
    saveAs: host.hasService('file.saveAs'),
    export: host.permissions.export,
  }
}

const EDITABLE = 'input, textarea, select, [contenteditable=""], [contenteditable="true"]'
const isEditable = (target: EventTarget | null) => target instanceof Element && target.closest(EDITABLE) != null
const dialogOpen = () => document.querySelector('.kui-dialog, .kui-dialog-backdrop') != null
const LAYERS = '[data-testid="photo-kit-layers"]'
/** How long Cmd+V waits for the browser's paste before it pastes the engine's clipboard. */
const PASTE_WAIT_MS = 600

type DropZone = 'canvas' | 'layers' | null

/** Whether the person was told this session that the browser keeps copies off the system clipboard. */
let toldLocal = false

export function PhotoFileIo({
  api,
  workspace,
  canvas,
  size,
  busy,
  editing,
}: {
  api: RefObject<FileIo | null>
  workspace: RefObject<HTMLDivElement | null>
  canvas: RefObject<KitCanvasHandle | null>
  size: { width: number; height: number } | null
  busy: Busy
  /** Text is being typed on the canvas: its own clipboard keys win. */
  editing: () => boolean
}) {
  const editor = useKitEditor()
  const { t, host } = editor
  const ui = useUi()
  const hostPick = useHostPick()
  const [exporting, setExporting] = useState(false)
  const [zone, setZone] = useState<DropZone>(null)
  const catcher = useRef<HTMLTextAreaElement>(null)
  // The size of the image this editor last put on the system clipboard: pasting it back is the engine's paste (its place kept).
  const ownCopy = useRef<string | null>(null)
  const exportApi = useRef<{ exportImage: (params: Record<string, unknown>) => Promise<Exported>; write: (format: ExportFormat, bytes: Uint8Array) => Promise<void> } | null>(null)
  const now = useRef({ editor, ui, size, hostPick, editing })
  now.current = { editor, ui, size, hostPick, editing }

  useLayoutEffect(() => {
    const surfaceBox = () => workspace.current?.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect() ?? null
    /** The document point under a client point. */
    const docPoint = (clientX: number, clientY: number) => {
      const view = canvas.current?.view()
      const box = surfaceBox()
      if (!view || !box) return null
      return { x: view.x + (clientX - box.left) / (view.zoom || 1), y: view.y + (clientY - box.top) / (view.zoom || 1) }
    }
    /** The document point at the middle of the view (where a paste lands, as Photoshop's does). */
    const viewCenter = () => {
      const box = surfaceBox()
      const point = box ? docPoint(box.left + box.width / 2, box.top + box.height / 2) : null
      const picture = now.current.size
      return point ?? (picture ? { x: picture.width / 2, y: picture.height / 2 } : { x: 0, y: 0 })
    }
    const allowed = (command: string) => {
      const { editor: e, ui: u } = now.current
      return e.enabled(command) && !u.state(`command:${command}`, e.byId.get(command)?.menu).denied
    }
    const run = (command: string, params: Record<string, unknown>) => now.current.editor.run(command, params)
    const doc = () => now.current.editor.session.doc

    /** Places `bytes` as a layer centred on `center`: an embedded object, or, for a paste, a pixel layer at 100% (`pixels`), in one step. */
    const placeBytes = async (name: string, bytes: Uint8Array, center: { x: number; y: number }, pixels = false): Promise<boolean> => {
      const answer = await run('file.placeEmbedded', { data: toBase64(bytes), name, center: [Math.round(center.x), Math.round(center.y)], ...(pixels ? { pixels: true, fit: false } : {}) })
      return answer !== null
    }
    const placeFiles = async (files: readonly File[], at: { x: number; y: number }) => {
      const placeable = files.filter((file) => isPlaceable(file))
      for (const file of files) if (!placeable.includes(file)) pushNotice(t('file.placeFailed'), t('drop.notImage', { name: file.name }))
      const centers = dropCenters(at, placeable.length)
      for (const [index, file] of placeable.entries()) {
        const [x, y] = centers[index]
        await placeBytes(file.name, new Uint8Array(await file.arrayBuffer()), { x, y })
      }
      workspace.current?.focus({ preventScroll: true })
    }

    const exportImage = async (params: Record<string, unknown>): Promise<Exported> => {
      const engine = doc()
      if (!engine) throw new Error('no document is open')
      const answer = JSON.parse(await engine.execute('file.exportImage', params)) as { data: string; width: number; height: number }
      return { bytes: fromBase64(answer.data), width: answer.width, height: answer.height }
    }
    const write = async (format: ExportFormat, bytes: Uint8Array) => {
      const path = await host.exportFile(exportPath(now.current.editor.session.path, format, t('dialog.untitled')), bytes)
      toast('success', t('export.done', { path }))
    }

    const enginePaste = () => {
      if (allowed('edit.paste')) void run('edit.paste', {})
      else pushNotice(t('error.title'), t('clipboard.noImage'))
    }
    const sizeKey = (width: number, height: number) => `${width}x${height}`
    /** Pastes `blob` (an image from the system clipboard) as a new layer, or the engine's copy when it is the one the editor put there. */
    const pasteImage = async (blob: Blob, name: string) => {
      const bitmap = await createImageBitmap(blob).catch(() => null)
      const key = bitmap ? sizeKey(bitmap.width, bitmap.height) : null
      bitmap?.close()
      if (key && key === ownCopy.current && allowed('edit.paste')) {
        void run('edit.paste', {})
        return
      }
      await placeBytes(nameFor({ name, type: blob.type }, t('clipboard.pastedImage')), new Uint8Array(await blob.arrayBuffer()), viewCenter(), true)
    }
    const imageOf = (data: DataTransfer | null): File | null => {
      if (!data) return null
      for (const item of Array.from(data.items ?? [])) {
        if (item.kind === 'file' && /^image\//i.test(item.type)) {
          const file = item.getAsFile()
          if (file) return file
        }
      }
      return Array.from(data.files ?? []).find((file) => isPlaceable(file)) ?? null
    }

    const copy = (command: 'edit.copy' | 'edit.copyMerged') => {
      const engine = doc()
      const done = run(command, {})
      const png = done.then(async (answer) => {
        if (answer === null || !engine) throw new Error('nothing copied')
        const image = JSON.parse(await engine.invoke<string>('clipboardImage')) as { data: string; width: number; height: number } | null
        if (!image) throw new Error('nothing copied')
        ownCopy.current = sizeKey(image.width, image.height)
        return new Blob([fromBase64(image.data).slice().buffer], { type: 'image/png' })
      })
      // The write starts in the key's (or click's) gesture, with the image to come: Safari asks for that.
      const write = typeof ClipboardItem !== 'undefined' && navigator.clipboard?.write ? navigator.clipboard.write([new ClipboardItem({ 'image/png': png })]) : Promise.reject(new Error('no clipboard'))
      void write.catch(async () => {
        // Said once a session: the copy still pastes inside Kition Photo.
        if ((await done.catch(() => null)) !== null && !toldLocal) {
          toldLocal = true
          toast('info', t('clipboard.copyLocal'))
        }
      })
      png.catch(() => undefined)
    }
    const paste = () => {
      const read = navigator.clipboard?.read
      if (!read) return enginePaste()
      void navigator.clipboard
        .read()
        .then(async (items) => {
          for (const item of items) {
            const type = item.types.find((kind) => kind.startsWith('image/'))
            if (type) return pasteImage(await item.getType(type), '')
          }
          enginePaste()
        })
        .catch(() => {
          if (allowed('edit.paste')) void run('edit.paste', {})
          else pushNotice(t('error.title'), t('clipboard.readLocal'))
        })
    }

    // Cmd+V: the browser's paste, caught by an editable field the keyboard is sent to for a moment
    // (a browser fires it only there), carries the system clipboard's image; Cmd+C and Cmd+Shift+C
    // copy onto it too; Cmd+Alt+Shift+W is Export As, Cmd+O Open where the host opens files.
    let pending: number | null = null
    const backToCanvas = () => workspace.current?.focus({ preventScroll: true })
    const settlePaste = () => {
      if (pending !== null) window.clearTimeout(pending)
      pending = null
    }
    const key = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey
      if (!command || event.defaultPrevented || isEditable(event.target) || dialogOpen() || now.current.editing() || !doc()) return
      const letter = event.code
      if (letter === 'KeyW' && event.shiftKey && event.altKey) {
        event.preventDefault()
        event.stopPropagation()
        if (fileAbilities(host).export) setExporting(true)
        return
      }
      if (letter === 'KeyO' && !event.shiftKey && !event.altKey && fileAbilities(host).open) {
        event.preventDefault()
        event.stopPropagation()
        void open()
        return
      }
      if (letter === 'KeyC' && !event.altKey) {
        const id = event.shiftKey ? 'edit.copyMerged' : 'edit.copy'
        if (!allowed(id)) return
        event.preventDefault()
        event.stopPropagation()
        copy(id)
        return
      }
      if (letter === 'KeyV' && !event.altKey && !event.shiftKey && !host.readOnly && catcher.current) {
        // Not prevented: the browser's own paste follows, into the catcher.
        event.stopPropagation()
        catcher.current.value = ''
        catcher.current.focus({ preventScroll: true })
        settlePaste()
        pending = window.setTimeout(() => {
          pending = null
          backToCanvas()
          enginePaste()
        }, PASTE_WAIT_MS)
      }
    }
    const pasted = (event: ClipboardEvent) => {
      const mine = event.target === catcher.current
      if (!mine && (isEditable(event.target) || dialogOpen() || now.current.editing())) return
      if (!doc() || host.readOnly) return
      event.preventDefault()
      settlePaste()
      if (mine) backToCanvas()
      const image = imageOf(event.clipboardData)
      if (image) void pasteImage(image, image.name)
      else enginePaste()
    }

    // Files dragged in from the computer: the canvas places them where they drop, the Layers panel
    // in the middle of the image; anywhere else the editor refuses them (the frame never navigates).
    let leaving: number | null = null
    const zoneOf = (event: DragEvent): DropZone => {
      if (host.readOnly || dialogOpen()) return null
      const target = event.target instanceof Element ? event.target : null
      if (target?.closest(LAYERS)) return 'layers'
      if (target && workspace.current?.contains(target)) return 'canvas'
      return null
    }
    const over = (event: DragEvent) => {
      if (!dragHasFiles(event.dataTransfer?.types)) return
      event.preventDefault()
      const where = zoneOf(event)
      if (event.dataTransfer) event.dataTransfer.dropEffect = where ? 'copy' : 'none'
      setZone(where)
      if (leaving !== null) window.clearTimeout(leaving)
      leaving = window.setTimeout(() => setZone(null), 200)
    }
    const dropped = (event: DragEvent) => {
      if (!dragHasFiles(event.dataTransfer?.types)) return
      event.preventDefault()
      const where = zoneOf(event)
      setZone(null)
      const files = Array.from(event.dataTransfer?.files ?? [])
      if (!where || !files.length) return
      const picture = now.current.size
      const at = where === 'canvas' ? docPoint(event.clientX, event.clientY) : picture ? { x: picture.width / 2, y: picture.height / 2 } : null
      if (at) void placeFiles(files, at)
    }

    const open = async () => {
      try {
        const path = await host.pick('open', [...OPEN_ACCEPT])
        if (path) await host.peer.request('host/open', { link: { path } })
      } catch (error) {
        reportError(t, t('file.openFailed'), error)
      }
    }
    const place = async () => {
      const { hostPick: picker } = now.current
      if (!picker.can) return
      try {
        const file = await picker.pick('place', [...PLACE_ACCEPT])
        if (file) await placeBytes(file.name, new Uint8Array(file.bytes), viewCenter())
      } catch (error) {
        reportError(t, t('file.placeFailed'), error)
      }
    }
    const saveAs = async () => {
      const engine = doc()
      if (!engine || !host.hasService('file.saveAs')) return
      try {
        const bytes = await engine.save()
        const name = now.current.editor.session.path.split(/[\\/]/).pop()
        const answer = await host.service('file.saveAs', { name, format: 'psd', bytes: bytes.slice().buffer })
        if (answer.path) toast('success', t('file.savedAs', { path: answer.path }))
      } catch (error) {
        reportError(t, t('error.title'), error)
      }
    }
    const quickExport = async () => {
      const job = busy.begin(t('busy.export'))
      try {
        const exported = await exportImage({ format: 'png' })
        if (busy.end(job)) return
        await write('png', exported.bytes)
      } catch (error) {
        if (!busy.end(job)) reportError(t, t('export.failed'), error)
      }
    }

    api.current = { exportAs: () => setExporting(true), quickExport, open, place, saveAs, copy, paste }
    exportApi.current = { exportImage, write }
    window.addEventListener('keydown', key, true)
    document.addEventListener('paste', pasted, true)
    window.addEventListener('dragover', over, true)
    window.addEventListener('drop', dropped, true)
    return () => {
      settlePaste()
      if (leaving !== null) window.clearTimeout(leaving)
      window.removeEventListener('keydown', key, true)
      document.removeEventListener('paste', pasted, true)
      window.removeEventListener('dragover', over, true)
      window.removeEventListener('drop', dropped, true)
    }
    // The handlers read the editor through `now`; they are set up once per host and canvas.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, t, busy])

  // The drop target's mark: the canvas or the Layers panel, outlined with what a drop does.
  useEffect(() => {
    const canvasArea = workspace.current
    const layers = document.querySelector<HTMLElement>(LAYERS)
    const mark = (element: HTMLElement | null | undefined, on: boolean) => {
      if (!element) return
      if (on) {
        element.dataset.drop = 'true'
        element.dataset.dropLabel = t('drop.place')
      } else {
        delete element.dataset.drop
        delete element.dataset.dropLabel
      }
    }
    mark(canvasArea, zone === 'canvas')
    mark(layers, zone === 'layers')
    return () => {
      mark(canvasArea, false)
      mark(layers, false)
    }
  }, [zone, t, workspace])

  return (
    <>
      <textarea ref={catcher} className="photo-paste-catcher" aria-hidden="true" tabIndex={-1} data-testid="photo-paste-catcher" />
      {exporting && size && exportApi.current ? <ExportDialog size={size} exportImage={exportApi.current.exportImage} write={exportApi.current.write} onClose={() => setExporting(false)} /> : null}
    </>
  )
}
