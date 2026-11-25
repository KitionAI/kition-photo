import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import {
  IconButton,
  KitElement,
  rgbToHex,
  EditorFrame,
  FloatingPanels,
  KitEditorProvider,
  createKitStrings,
  kitLeases,
  shortcutMatches,
  toast,
  useKitEditorState,
  useKitSession,
  type DockAreaHandle,
  type FloatingPanel,
  type FrameTab,
  type HostConnection,
  type KitViewState,
  type KitCanvasHandle,
  type KitCanvasView,
  type KitPointer,
  type KitTool,
  type Phrases,
  type Strings,
  type RasterRect,
  type RasterView,
  type RasterViewportStats,
} from '@kition/sdk/editor'
import { engineManifest } from './engine'
import { CATALOGS } from './strings'
import { describe, pictureArea, picturePanBounds } from './photo'
import { CanvasScrollbars } from './CanvasScrollbars'
import { dashed, forgetStrokeEnd, photoCanvasTool, transformTool, ALT_PICKS, type CropFrame, type PendingClicks, type PhotoToolEnv } from './tools/canvas'
import { brushSizeStep, cropParams, digitPercent, hardnessStep, nudgeStep, type DigitEntry } from './tools/canvas/geometry'
import { livePreview } from './tools/canvas/live'
import { createHeldKeys } from './tools/held'
import { BrushCursor, outlineDiameter, type CanvasPoint } from './tools/BrushCursor'
import { previewEngine, useFreeTransform } from './tools/useFreeTransform'
import { Nudges, digitTarget } from './tools/canvasKeys'
import { toBitmap } from './panels/thumbnails'
import { liveStroke } from './tools/canvas/stroke'
import { usePhotoPrefs, type PhotoPrefs } from './prefs'
import { paintsPixels, rasterizeNeeded, rasterizeThenPaint, withRasterizePrompt, type RasterizeAsk } from './tools/rasterize'
import { RasterizePrompt } from './tools/RasterizePrompt'
import { TOOL_FIELDS, optionsFor, toolParams, type ToolOptions } from './tools/options'
import { PhotoToolRail, type ToolKeys } from './tools/PhotoToolRail'
import { toolFor, type PhotoToolId } from './tools/catalog'
import { NO_EXTRAS, NO_MARKS, usePhotoOverlays, type CanvasExtras, type CanvasMarks } from './overlays'
import { OptionsBar } from './options/OptionsBar'
import { useToolOptions } from './options/store'
import { PhotoDock, dockGroupId } from './panels/dock'
import { createLive, trackPointer, useLive, type DocPoint } from './panels/live'
import { usePhotoColors } from './colors'
import { useMaskTarget } from './panels/useMaskTarget'
import { pendingFontChoices, pendingTypeFont } from './panels/useTypeStyle'
import { TextField, type TextDraft, type TextFieldActions } from './TextField'
import { TypeSession, sessionKey } from './type/session'
import { TypeEditor } from './type/TypeEditor'
import { TypeEditingContext, type TypeEditing } from './type/context'
import { dragBox, isBoxDrag, type Range as TextRange } from './type/text'
import { DEFAULT_WINDOWS, windowCommand, zoomStep, type DockTabs } from './menus/kitMenus'
import { isHiddenCommand } from './menus/hidden'
import { PhotoStatus } from './PhotoStatus'
import { PaintLease, layerScope, useLayerObjectIds, withPaintLease } from './layerLeases'
import { PhotoMenuKeys, photoMenuTabs, type MenuKeys } from './menus/photoMenus'
import { HelpDialog, type HelpTopic } from './menus/HelpDialog'
import { HomeTab } from './HomeTab'
import { usePhotoViewState } from './viewState'
import { PhotoCanvas } from './PhotoCanvas'
import { DialogsProvider, PhotoDialogs, type Dialogs } from './dialogs/DialogHost'
import { ColorPickerOutlet, ColorPickerProvider, type SampleAt } from './color/ColorPickerHost'
import { ColorWell } from './color/ColorWell'
import { GradientEditorOutlet, GradientEditorProvider } from './gradient/GradientEditorHost'
import { BrushPickerPopover, BrushPresets, BrushSettingsPanel, type BrushOptions } from './brushes/BrushPicker'
import { SMOOTHING_TOOLS } from './tools/options'
import manifest from '../../kit.json'
import { Busy } from './io/busy'
import { BusyIndicator } from './io/BusyIndicator'
import { ErrorNotices, reportError } from './io/ErrorNotices'
import { PhotoFileIo, fileAbilities, type FileIo } from './io/PhotoFileIo'
import { withoutEllipsis } from './dialogs/routes'
import { CanvasMenu } from './canvasMenu/CanvasMenu'
import { PATH_TOOLS, canvasMenuRows, type CanvasMenuContext, type CanvasMenuRow } from './canvasMenu/rows'
import { Rulers } from './guides/Rulers'
import { useDocGuides, useViewSettings, withGuides, type GuideControl } from './guides/useGuides'
import { NO_GUIDES, SNAP_DISTANCE, snapMove, snapPoint, snapSource, type Box, type SnapLine, type Snapper } from './guides/snap'
import { VIEW_SETTING_ROWS, gridSpacing, shown as extraShown, viewChecked, viewRow } from './guides/view'
import type { GuideDraft } from './guides/draw'
import { NO_PATHS, createPathStore, partAt as partAtPoint, prunedSelection } from './paths/store'
import type { PathData } from './paths/bezier'
import { deleteSelectedAnchors, editPointAt, nudgeAnchors } from './tools/canvas/pathTools'
import { PHOTO_MENUS } from './menus/catalog'


const ArrowLeftRight = TesseraIcons.ArrowLeftRightIcon

/** Tools whose Cmd (Ctrl) is their own, so holding it does not turn a press into the Move tool's (upstream's list). */
const CMD_IS_OWN: ReadonlySet<PhotoToolId> = new Set<PhotoToolId>(['Move', 'Type', 'VerticalType', 'Hand', 'Zoom', 'Crop', 'Slice', 'SliceSelect', 'Pen', 'PathSelection', 'DirectSelection', 'Rectangle', 'EllipseShape', 'Triangle', 'Polygon', 'Line', 'CustomShape'])
/** Tools whose arrow keys nudge the selection's outline while there is one. */
const OUTLINE_TOOLS: ReadonlySet<PhotoToolId> = new Set<PhotoToolId>(['RectMarquee', 'EllipseMarquee', 'Lasso', 'PolygonLasso', 'MagneticLasso', 'ObjectSelection', 'QuickSelection', 'MagicWand'])
const ARROWS: Readonly<Record<string, readonly [number, number]>> = { arrowleft: [-1, 0], arrowright: [1, 0], arrowup: [0, -1], arrowdown: [0, 1] }
/** Where arrow and number keys stay the control's own (a menu, a slider, a list). */
const OWN_KEYS = 'input, select, textarea, [role="menu"], [role="menubar"], [role="menuitem"], [role="slider"], [role="listbox"], [role="option"], [role="tree"], [role="treeitem"], [role="grid"], [role="row"], [role="tablist"], [role="tab"], [role="radiogroup"], [role="scrollbar"], .kui-popover'
/** The tools whose arrow keys and Delete act on the selected anchors of a path (plan 2026-11-24, stage 7). */
const PATH_EDIT_TOOLS: ReadonlySet<string> = new Set(['Pen', 'PathSelection', 'DirectSelection', 'ConvertPoint'])
/** The menu rows View answers itself, with their keys (Cmd+R Rulers, Cmd+; Guides, Cmd+' Grid…). */
const VIEW_KEY_ROWS = PHOTO_MENUS.filter((row) => row.shortcut && VIEW_SETTING_ROWS.has(row.command))
const RotateCcw = TesseraIcons.RotateCcwIcon
const SquareDashed = TesseraIcons.SquareDashedIcon

export function App({ host, phrase }: { host: HostConnection; phrase: Phrases }) {
  // Declared tool labels are upstream's English labels: a string Photo's catalogs lack is looked up in upstream's translations.
  const t = useMemo<Strings>(() => {
    const own = createKitStrings(CATALOGS, host.init.locale)
    return (key, values) => {
      const text = own(key, values)
      return text === key ? phrase(key) : text
    }
  }, [host, phrase])
  const engine = useMemo(engineManifest, [])
  // The person's view (selection, collapsed groups, …), kept by the host after each save (see `viewState.ts`).
  const keptView = useRef<KitViewState | undefined>(undefined)
  const sessionOptions = useMemo(() => ({ viewState: () => keptView.current }), [])
  const kitSession = useKitSession(host, engine, describe, sessionOptions)
  usePhotoViewState(host, kitSession, keptView)
  // The editor runs every command through the mask target's session (see `useMaskTarget`).
  const { session: photoSession, maskTarget, setMaskTarget, aim } = useMaskTarget(kitSession)
  const commandSequence = useRef(0)
  const pendingCommands = useRef(new Set<Promise<unknown>>())
  // A command still running after half a second shows the progress bar with Cancel (plan
  // 2026-11-24, stage 6); a cancelled one's step is taken away when it ends (`io/busy.ts`).
  const busy = useMemo(() => new Busy(), [])
  const titleOf = useRef<(command: string) => string>((command) => command)
  const photoRun = useRef(photoSession)
  photoRun.current = photoSession
  const runSession = useCallback((...args: Parameters<typeof photoSession.run>) => {
    commandSequence.current += 1
    const [id, params] = args
    // A dialog shows its own progress (its preview), and History's own steps are not ones to cancel.
    const tracked = !document.querySelector('.kui-dialog') && !id.startsWith('history.') && !(typeof params?.coalesce === 'string' && params.coalesce.startsWith('photo-dialog'))
    const job = tracked ? busy.begin(titleOf.current(id), id !== 'edit.undo' && id !== 'edit.redo') : null
    const before = photoRun.current.doc?.info.revision ?? 0
    const command = photoSession.run(...args)
    pendingCommands.current.add(command)
    const finished = () => { pendingCommands.current.delete(command) }
    void command.then(finished, finished)
    if (job) {
      void command.then(
        async (answer) => {
          if (!busy.end(job) || answer === null) return
          // Cancelled: the step it made leaves the history (Redo included), as a cancelled dialog's preview does.
          const session = photoRun.current
          if ((session.doc?.info.revision ?? 0) === before) return
          if ((await session.run('history.deleteState', {})) === null) await session.run('edit.undo', {})
        },
        () => void busy.end(job),
      )
    }
    return command
  }, [photoSession.run, busy])
  const session = { ...photoSession, run: runSession }
  // The engine's errors in the editor's words (plan 2026-11-24, stage 6), never its English with command ids.
  const editorOptions = useMemo(() => ({ report: (title: string, error: unknown) => void reportError(t, title, error) }), [t])
  const editor = useKitEditorState(session, host, t, 'photo', phrase, undefined, editorOptions)
  titleOf.current = (command) => {
    const title = editor.byId.get(command)?.title
    return title ? phrase(withoutEllipsis(title)) : command
  }
  const { doc, change, state } = session
  // Type edited on the canvas (plan 2026-11-24, stage 4): one session at a time, its commands in one history step.
  const typeRun = useRef(editor.run)
  typeRun.current = editor.run
  const typeQuery = useRef(editor.query)
  typeQuery.current = editor.query
  const typeSession = useMemo(
    () =>
      new TypeSession({
        run: (command, params) => typeRun.current(command, params),
        query: (command, params) => typeQuery.current(command, params),
        revision: () => docNow.current?.info.revision ?? 0,
      }),
    [],
  )
  const typeTarget = useLive(typeSession.target)
  const typeEditing = typeTarget != null
  const canvas = useRef<KitCanvasHandle>(null)
  const workspace = useRef<HTMLDivElement>(null)
  const viewMemory = useRef<RasterView | null>(null)
  const dock = useRef<DockAreaHandle>(null)
  // Which tab each dock group shows, read when the Window menu opens or runs a row.
  const dockTabs = useCallback<DockTabs>((group) => dock.current?.activeTab(dockGroupId(group)) ?? null, [])
  const [tool, setTool] = useState<PhotoToolId>('Brush')
  const [toolGeneration, setToolGeneration] = useState(0)
  const toolGenerationNow = useRef(0)
  const [optionsState, setToolOptions] = useToolOptions()
  // Preferences › General (Cmd+K): saved, Show Transform Controls goes to the Move tool at once.
  const [prefs, setPrefs] = usePhotoPrefs()
  const savePrefs = useCallback(
    (next: PhotoPrefs) => {
      setPrefs(next)
      setToolOptions('Move', { showTransform: next.showTransform })
    },
    [setPrefs, setToolOptions],
  )
  // The Crop tool's frame (plan 2026-11-24, stage 2): the whole canvas when the tool is chosen.
  const [crop, setCrop] = useState<CropFrame | null>(null)
  const cropNow = useRef<CropFrame | null>(null)
  cropNow.current = crop
  const [textDraft, setTextDraft] = useState<TextDraft | null>(null)
  const [textPending, setTextPending] = useState(false)
  const [textDirty, setTextDirty] = useState(false)
  const textSave = useRef<Promise<void> | null>(null)
  const textSaveTarget = useRef<{ draft: number; revision: number | null; command: number } | null>(null)
  const textSaveAgain = useRef(false)
  const textDraftId = useRef(0)
  const textNavigation = useRef(0)
  const focusText = useRef(true)
  const focusAfterText = useRef(false)
  useLayoutEffect(() => {
    if ((textDraft && !textPending) || !focusAfterText.current) return
    focusAfterText.current = false
    // The input must be removed before focus moves, or its blur can commit the draft again.
    // Restore focus in this commit, without waiting for the worker or another animation frame.
    workspace.current?.focus()
  }, [textDraft, textPending])
  const [cloneSource, setCloneSource] = useState<{ x: number; y: number } | null>(null)
  const [zoom, setZoom] = useState<number | null>(null)
  const [stats, setStats] = useState<RasterViewportStats | null>(null)
  // Scrolling stops a margin past the picture: the view never leaves it for empty canvas.
  const [shown, setShown] = useState<{ zoom: number; width: number; height: number } | null>(null)
  const panBounds = useMemo(() => picturePanBounds(state?.width ?? 0, state?.height ?? 0, shown), [state?.width, state?.height, shown])
  const [windows, setWindows] = useState(DEFAULT_WINDOWS)
  const [help, setHelp] = useState<HelpTopic | null>(null)
  // The brush preset picker's place (the brush chip's, a right click's), and the brush windows open (plan 2026-11-24, stage 5).
  const [brushPicker, setBrushPicker] = useState<{ x: number; y: number } | null>(null)
  const [floating, setFloating] = useState<readonly string[]>([])
  const [floatingFocus, setFloatingFocus] = useState<string | null>(null)
  const toggleFloating = useCallback((id: string) => {
    setFloating((open) => (open.includes(id) ? open.filter((item) => item !== id) : [...open, id]))
    setFloatingFocus(id)
  }, [])
  const [moving, setMoving] = useState(false)
  // The "Rasterize?" prompt a press on a type, shape, object or fill layer opened; one at a time.
  const [rasterize, setRasterize] = useState<(RasterizeAsk & { generation: number }) | null>(null)
  const rasterizeOpen = useRef(false)
  rasterizeOpen.current = rasterize != null
  const textActions = useRef<TextFieldActions | null>(null)
  const menuKeys = useRef<MenuKeys | null>(null)
  const toolKeys = useRef<ToolKeys | null>(null)
  const pending = useRef<PendingClicks | null>(null)
  const cancelToolDrafts = useCallback(() => {
    // Released gestures can still await a query/lease. Invalidate their command continuations,
    // even if permission returns before the asynchronous work completes.
    setToolGeneration(++toolGenerationNow.current)
    pending.current?.cancel()
    setCrop(null)
    setRasterize(null)
    rasterizeOpen.current = false
  }, [])
  useLayoutEffect(() => {
    // A file-stamp refresh replaces the engine without remounting App. Proposals and
    // asynchronous tool continuations still belong to the engine that created them.
    cancelToolDrafts()
    typeSession.drop()
    setCloneSource(null)
    forgetStrokeEnd()
    transformNow.current.forget()
    // The canvas builds a new viewport for a new document: the old one must never draw again (it
    // would put the previous document's pixels back). The next pointer event brings the new one.
    viewport.current = null
  }, [doc, cancelToolDrafts, typeSession])
  // Someone else's change (an undo, a panel, an agent) ends the edit where it is; so does another
  // layer becoming the active one (Photoshop commits then).
  useEffect(() => typeSession.checkExternal(), [change.revision, typeSession])
  // A denied tool drops its drafts and commits text being edited (a stable callback: the canvas runs it from an effect).
  const denyTools = useCallback(() => {
    cancelToolDrafts()
    void typeSession.commit()
  }, [cancelToolDrafts, typeSession])
  const activeBefore = useRef<number | null | undefined>(undefined)
  useEffect(() => {
    const before = activeBefore.current
    activeBefore.current = state?.activeLayer
    if (typeSession.active && before === typeSession.layer && state?.activeLayer !== typeSession.layer) void typeSession.commit()
  }, [state?.activeLayer, typeSession])
  useLayoutEffect(() => {
    typeSession.onEnd = () => {
      // The keyboard goes back to the canvas when it was the session's.
      if (document.activeElement instanceof HTMLTextAreaElement && document.activeElement.dataset.testid === 'photo-type-input') workspace.current?.focus({ preventScroll: true })
    }
  }, [typeSession])
  const { run, query } = editor
  // Keys work as soon as the picture shows: the editor takes the keyboard focus once, on open.
  const focused = useRef(false)
  useEffect(() => {
    if (!state || focused.current) return
    focused.current = true
    const active = document.activeElement
    if (active && active !== document.body && active !== document.documentElement) return
    window.focus()
    workspace.current?.focus({ preventScroll: true })
  }, [state])
  // The foreground and background colours live in the engine (`usePhotoColors`).
  const sayLocalSwatches = useCallback(() => toast('info', t('swatches.local')), [t])
  const colors = usePhotoColors(doc, change.revision, query, sayLocalSwatches)
  // The pointer over the document and the canvas's view change on every move: the panels that
  // read them (Info, Navigator) subscribe, the editor does not redraw.
  const pointer = useMemo(() => createLive<DocPoint>(null), [])
  const view = useMemo(() => createLive<KitCanvasView | null>(null), [])
  const options = optionsFor(optionsState, tool)
  // The canvas tools read the options when they act, so a scrubbed field rebuilds no tool.
  const optionsNow = useRef(options)
  optionsNow.current = options
  const activeLayer = state?.layers.find((layer) => layer.id === state.activeLayer)
  const quickMask = state?.channels?.quickMask != null
  // The canvas's marks (crop frame, transform box, the selected layer's mark), read on every frame.
  const marks = useRef<CanvasMarks>(NO_MARKS)
  const viewport = useRef<Parameters<KitTool['begin']>[1]['viewport'] | null>(null)
  const redraw = useCallback(() => viewport.current?.draw(), [])
  // View's rulers, guides, grid and snapping, and the paths the path tools edit (plan 2026-11-24, stage 7).
  const [viewSettings, setViewSettings] = useViewSettings()
  const viewSettingsNow = useRef(viewSettings)
  viewSettingsNow.current = viewSettings
  const guides = useDocGuides(doc, change.revision)
  const guidesNow = useRef(guides)
  guidesNow.current = guides
  const extras = useRef<CanvasExtras>(NO_EXTRAS)
  const paths = useMemo(createPathStore, [])
  const pathTool = PATH_TOOLS.has(tool)
  // View's extras as the canvas draws them, set as the editor renders; a change of them gives the
  // canvas a new decoration, which it draws at once (even before any pointer has been over it).
  const activeBounds = state?.activeBounds ?? null
  extras.current = {
    ...extras.current,
    settings: viewSettings,
    guides,
    resolution: state?.resolution ?? 72,
    paths: pathTool ? paths : null,
    tool,
    layerEdges: activeBounds ? { x0: activeBounds.x0, y0: activeBounds.y0, x1: activeBounds.x1, y1: activeBounds.y1 } : null,
  }
  const [guideDraftShown, setGuideDraftShown] = useState<GuideDraft | null>(null)
  const extrasVersion = JSON.stringify([viewSettings, guides, state?.resolution, pathTool, tool, activeBounds, guideDraftShown])
  const overlays = usePhotoOverlays(doc, change, state?.hasSelection ?? false, zoom, marks, extras, extrasVersion)
  /** A guide being dragged (out of a ruler, or moved), drawn at once (the ruler's drag may come before the canvas has seen the pointer, so the canvas is also given a new decoration). */
  const setGuideDraft = useCallback(
    (draft: GuideDraft | null) => {
      extras.current = { ...extras.current, guideDraft: draft }
      setGuideDraftShown(draft)
      redraw()
    },
    [redraw],
  )
  // The paths the path tools edit: the work path and the active shape layer's, read again after every change.
  const activeKind = state?.layers.find((layer) => layer.id === state.activeLayer)?.kind
  useEffect(() => {
    if (!doc || !pathTool) {
      paths.set(NO_PATHS)
      return undefined
    }
    let current = true
    const shapeLayer = activeKind === 'Shape' ? (state?.activeLayer ?? null) : null
    const read = (command: string, params: Record<string, unknown>) => doc.execute(command, params).then((answer) => (JSON.parse(answer) as { path?: PathData }).path ?? null, () => null)
    void Promise.all([read('path.info', { name: 'work' }), shapeLayer != null ? read('shape.info', { layer: shapeLayer }) : Promise.resolve(null)]).then(([work, shape]) => {
      if (!current) return
      const view = paths.get()
      const next = { ...view, work, shape: shape && shapeLayer != null ? { layer: shapeLayer, path: shape } : null }
      paths.set({ ...next, selected: prunedSelection(next) })
    })
    return () => {
      current = false
    }
  }, [doc, change.revision, pathTool, state?.activeLayer, activeKind, paths])
  useEffect(() => paths.subscribe(redraw), [paths, redraw])
  // The keys held now, the pointer over the canvas (the brush outline follows it), and the cursor a tool asks for.
  const held = useMemo(createHeldKeys, [])
  const keys = useLive(held)
  const canvasPointer = useMemo(() => createLive<CanvasPoint>(null), [])
  const [toolCursor, setToolCursor] = useState<string | null>(null)
  const docNow = useRef(doc)
  docNow.current = doc
  const say = useCallback((key: string) => toast('info', t(key)), [t])
  const sessionRunNow = useRef<(command: string, params: Record<string, unknown>) => Promise<unknown>>(() => Promise.resolve(null))
  const revisionNow = useCallback(() => docNow.current?.info.revision ?? 0, [])
  // The box's drags recompute the marks (`syncMarks`, defined below) and redraw them.
  const syncMarksNow = useRef<() => void>(() => undefined)
  const remark = useCallback(() => syncMarksNow.current(), [])
  const transform = useFreeTransform({ info: () => stateNow.current, run: (command, params) => sessionRunNow.current(command, params), revision: revisionNow, say, redraw: remark })
  const transformNow = useRef(transform)
  transformNow.current = transform
  const nudges = useMemo(() => new Nudges((command) => previewEngine(command, (name, params) => sessionRunNow.current(name, params), revisionNow), () => say('nudge.failed')), [revisionNow, say])
  useEffect(() => () => nudges.close(), [nudges])
  const fg = colors.foreground
  const bg = colors.background
  // The Color Picker's eyedropper: the picture's colour under a page point over the canvas.
  const sampleAt = useCallback<SampleAt>(
    (clientX, clientY) => {
      const surface = workspace.current?.querySelector('[data-testid="kit-canvas-surface"]')
      const shownView = canvas.current?.view()
      const size = stateNow.current
      if (!surface || !shownView || !size) return null
      const box = surface.getBoundingClientRect()
      if (clientX < box.left || clientX >= box.right || clientY < box.top || clientY >= box.bottom) return null
      const x = Math.floor(shownView.x + (clientX - box.left) / (shownView.zoom || 1))
      const y = Math.floor(shownView.y + (clientY - box.top) / (shownView.zoom || 1))
      if (x < 0 || y < 0 || x >= size.width || y >= size.height) return Promise.resolve(null)
      return query<number[]>('file.sampleColor', { x, y, size: 'point', sampleLayer: 'all' }).then((rgba) =>
        Array.isArray(rgba) && rgba.length >= 3 ? rgbToHex(rgba.slice(0, 3).map((value) => value * 255) as [number, number, number]) : null,
      )
    },
    [query],
  )
  // The canvas tools read the colours when they act, so a colour field's drag rebuilds no tool.
  const colorsNow = useRef(colors)
  colorsNow.current = colors
  // The tools read the document's state and the mask target when they act, so neither rebuilds a tool.
  const stateNow = useRef(state)
  stateNow.current = state
  sessionRunNow.current = (command, params) => session.run(command, params)
  const maskTargetNow = useRef(maskTarget)
  maskTargetNow.current = maskTarget
  // A pixel tool's strokes lease the layer they paint while the document is co-edited (`layerLeases.ts`).
  const leases = useMemo(() => kitLeases(host), [host])
  const paintLease = useMemo(() => new PaintLease(leases, (holder) => toast('info', t('lease.painting', { name: holder || t('lease.someone') }))), [leases, t])
  useEffect(() => () => paintLease.release(), [paintLease])
  const layerObjects = useLayerObjectIds(doc, change.revision, leases.available)
  const activeObject = activeLayer?.id != null ? layerObjects.get(activeLayer.id) : undefined
  useEffect(() => paintLease.setScope(activeObject ? layerScope(activeObject) : null), [paintLease, activeObject])
  // A tool's command: false when it failed, which a toast explains (the same failure again refreshes
  // it), or when the layer's lease was refused (which `paintLease` explains).
  const runTool = useCallback(
    async (command: string, params: Record<string, unknown>, damage: RasterRect | null = null, current: () => boolean = () => true) => {
      if (!current()) return false
      if (paintLease.waiting && !(await paintLease.allowed())) return false
      if (!current()) {
        paintLease.idle()
        return false
      }
      const done = await session.run(command, params, damage).then(
        (result) => result !== null,
        (error: unknown) => {
          toast('error', t('commandFailed'), error instanceof Error ? error.message : String(error))
          return false
        },
      )
      paintLease.idle()
      return done
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- session.run is stable
    [t, paintLease],
  )
  // [ and ] step the brush's size through Photoshop's sizes, Shift+[ and Shift+] its hardness by quarters.
  const stepBrush = (direction: 1 | -1, hardness: boolean) => {
    const fields = TOOL_FIELDS[tool]
    if (hardness) {
      if (fields.includes('hardness')) setToolOptions(tool, { hardness: hardnessStep(options.hardness, direction) })
    } else if (fields.includes('size')) setToolOptions(tool, { size: brushSizeStep(options.size, direction) })
  }
  // Number keys: Opacity (Shift: Flow), two quick keys an exact value; other tools the layer's.
  const digitEntry = useRef<DigitEntry>(null)
  const typeDigit = (digit: number, shift: boolean) => {
    const typed = digitPercent(digitEntry.current, digit, Date.now())
    digitEntry.current = typed.entry
    const target = digitTarget(tool, shift)
    if (!target) return
    if ('option' in target) {
      const [low, high] = target.option === 'flow' ? [1, 100] : [0, 100]
      setToolOptions(tool, { [target.option]: Math.min(high, Math.max(low, typed.percent)) })
      return
    }
    const active = stateNow.current?.layers.find((layer) => layer.id === stateNow.current?.activeLayer)
    if (!active || active.background) return
    void run('layer.setProps', { [target.layer]: typed.percent / 100 })
  }

  // The crop frame and Free Transform's box, as the canvas marks them; the selected layer's mark
  // (the Move tool's: Show Transform Controls' handles, or a thin outline).
  const syncMarks = useCallback(() => {
    const box = transformNow.current.boxNow.current
    const now = stateNow.current
    const active = now?.layers.find((layer) => layer.id === now.activeLayer)
    const frame = cropNow.current
    const bounds = now?.activeBounds
    const markLayer = markNow.current.tool === 'Move' && !markNow.current.moving && bounds && active && !active.background
    const next: CanvasMarks = {
      crop: markNow.current.tool === 'Crop' && frame ? { frame, thirds: markNow.current.thirds } : null,
      transform: box
        ? { quad: box.quad, handles: true, reference: box.reference }
        : markLayer
          ? { quad: [{ x: bounds.x0, y: bounds.y0 }, { x: bounds.x1, y: bounds.y0 }, { x: bounds.x1, y: bounds.y1 }, { x: bounds.x0, y: bounds.y1 }], handles: markNow.current.handles && !active.locks.position && !active.locks.all, reference: null }
          : null,
      hideAnts: marks.current.hideAnts,
    }
    // Redraw only when a mark changed (the editor renders often; the canvas need not).
    if (JSON.stringify(next) === JSON.stringify(marks.current)) return
    marks.current = next
    redraw()
  }, [redraw])
  const markNow = useRef({ tool, moving, thirds: options.cropOverlay, handles: options.showTransform })
  markNow.current = { tool, moving, thirds: options.cropOverlay, handles: options.showTransform }
  syncMarksNow.current = syncMarks
  useLayoutEffect(syncMarks)
  /** A crop frame from a drag: drawn at once, and kept (for the options bar) when `settled`. */
  const updateCrop = useCallback(
    (frame: CropFrame | null, settled: boolean) => {
      cropNow.current = frame
      syncMarks()
      if (settled) setCrop(frame)
    },
    [syncMarks],
  )
  const applyCropNow = useRef<() => void>(() => undefined)

  // The Type tools' presses and the Move tool's double click on type (defined below, read when they act).
  type TypeContextArg = Parameters<KitTool['begin']>[1]
  const typePressNow = useRef<(pointer: KitPointer, orientation: 'horizontal' | 'vertical', context: TypeContextArg) => ReturnType<KitTool['begin']>>(() => undefined)
  const typeHoverNow = useRef<(pointer: KitPointer, context: TypeContextArg) => void>(() => undefined)
  const editTextAtNow = useRef<(pointer: KitPointer) => Promise<void>>(() => Promise.resolve())

  // Snapping (plan 2026-11-24, stage 7): to the guides and the grid, to other layers and the
  // document as View › Snap To asks, and with Smart Guides on, the Move tool's drag to the other
  // layers' and the document's edges and centres, with magenta lines. Ctrl held suspends it.
  const snapper = useMemo<Snapper>(() => {
    const source = (moving: number | null | undefined, smart: boolean) => {
      const settings = viewSettingsNow.current
      const now = stateNow.current
      if (!now || held.get().ctrl) return null
      const to = settings.snap ? settings.snapTo : { guides: false, grid: false, layers: false, documentBounds: false }
      const smartOn = smart && extraShown(settings, 'smartGuides')
      const layers = to.layers || smartOn
      const boxes: Box[] = layers ? now.layers.filter((layer) => layer.visible && layer.id !== moving && layer.kind !== 'Group' && layer.bounds && layer.bounds[2] > 0 && layer.bounds[3] > 0).map((layer) => ({ x0: layer.bounds![0], y0: layer.bounds![1], x1: layer.bounds![0] + layer.bounds![2], y1: layer.bounds![1] + layer.bounds![3] })) : []
      if (!settings.snap && !smartOn) return null
      return snapSource({
        guides: extraShown(settings, 'guides') ? guidesNow.current : NO_GUIDES,
        layers: boxes,
        width: now.width,
        height: now.height,
        grid: extraShown(settings, 'grid') ? gridSpacing(now.resolution).minor : null,
        to: { ...to, layers, documentBounds: to.documentBounds || smartOn },
      })
    }
    // The lines a snap shows: what it met besides a guide or the grid (those show already).
    const report = (lines: readonly SnapLine[]) => {
      const shownLines = lines.filter((line) => line.kind === 'layer' || line.kind === 'document')
      if (!shownLines.length && !extras.current.snapLines.length) return
      extras.current = { ...extras.current, snapLines: shownLines }
      redraw()
    }
    return {
      point: (p, zoom) => {
        const from = source(null, false)
        const answer = from ? snapPoint(p, from, SNAP_DISTANCE / (zoom || 1)) : { ...p, lines: [] }
        report(answer.lines)
        return answer
      },
      move: (box, dx, dy, zoom, moving) => {
        const from = source(moving, true)
        const answer = from ? snapMove(box, dx, dy, from, SNAP_DISTANCE / (zoom || 1)) : { dx, dy, lines: [] }
        report(answer.lines)
        return answer
      },
      done: () => report([]),
    }
  }, [held, redraw])
  // Guides to grab with the Move tool (or Cmd held): moved, or dragged back to a ruler to delete them.
  const guideControl = useMemo<GuideControl>(
    () => ({
      guides: () => guidesNow.current,
      grabbable: () => extraShown(viewSettingsNow.current, 'guides') && !viewSettingsNow.current.lockGuides,
      size: () => {
        const shownView = canvas.current?.view()
        return shownView ? { width: shownView.width, height: shownView.height } : null
      },
      draft: setGuideDraft,
      move: (orientation, index, position) => void typeRun.current('view.moveGuide', { orientation, index, position }),
      remove: (orientation, index) => void typeRun.current('view.deleteGuide', { orientation, index }),
      setCursor: setToolCursor,
    }),
    [setGuideDraft],
  )

  // The session's `run` takes a repaint area, which tells the canvas what to redraw; a failed tool
  // command says why. A tool that paints pixels asks first to rasterize a layer that has none.
  const env = useMemo<PhotoToolEnv>(() => ({
        get options() {
          return optionsNow.current
        },
        get foreground() {
          return colorsNow.current.foreground
        },
        get background() {
          return colorsNow.current.background
        },
        // Read when the tool acts: a change of active layer (Auto-Select's, mid-drag) rebuilds no
        // tool, so the canvas keeps the drag under way.
        get layer() {
          const now = stateNow.current
          const active = now?.layers.find((layer) => layer.id === now.activeLayer)
          if (!active) return null
          return { id: active.id, kind: active.kind, bounds: now?.activeBounds ?? null, locked: active.background || active.locks.position || active.locks.all, background: active.background, opacity: active.opacity ?? 1 }
        },
        get hasLayerEffects() {
          return stateNow.current?.layers.some((layer) => layer.visible && layer.effects?.enabled && layer.effects.items.some((effect) => effect.enabled)) ?? false
        },
        run: (command, params, damage) => runTool(command, params, damage, () => toolGeneration === toolGenerationNow.current),
        query,
        get selection() {
          return stateNow.current?.selectionBounds ?? null
        },
        get documentSize() {
          return { width: stateNow.current?.width ?? 0, height: stateNow.current?.height ?? 0 }
        },
        pick: (rgb, toBackground) => {
          if (toolGeneration === toolGenerationNow.current) colorsNow.current.set({ [toBackground ? 'background' : 'foreground']: rgbToHex(rgb) })
        },
        proposeCrop: (area) => updateCrop({ ...area, angle: 0, initial: false }, true),
        crop: {
          get frame() {
            return cropNow.current
          },
          update: updateCrop,
          commit: () => applyCropNow.current(),
        },
        transform: {
          get box() {
            return transformNow.current.boxNow.current
          },
          start: () => transformNow.current.start(),
          update: (quad, reference, settled) => transformNow.current.update(quad, reference, settled),
        },
        preview: (command) => livePreview(previewEngine(command, (name, params) => sessionRunNow.current(name, params), revisionNow), () => toast('error', t('commandFailed'))),
        setCursor: setToolCursor,
        get selectionOutline() {
          return overlays.outline.current
        },
        hideSelection: (hidden) => {
          marks.current = { ...marks.current, hideAnts: hidden }
          redraw()
        },
        say,
        redraw,
        layerImage: (layer) => {
          const open = docNow.current
          return open ? open.invoke<Uint8Array>('layerThumbnail', layer, 1024).then(toBitmap, () => null) : Promise.resolve(null)
        },
        typePress: (pointer, orientation, context) => typePressNow.current(pointer, orientation, context),
        typeHover: (pointer, context) => typeHoverNow.current(pointer, context),
        editTextAt: (pointer) => void editTextAtNow.current(pointer),
        cloneSource,
        setCloneSource,
        pending,
        moving: setMoving,
        // Plan 2026-11-24, stage 3: the engine paints a brush stroke while it is drawn
        // (`bindings/wasm` `stroke.rs`), aimed at the mask target as the commit is.
        liveStroke: (command, params, viewport, failed) => {
          const open = docNow.current
          if (!open || toolGeneration !== toolGenerationNow.current) return null
          return liveStroke(
            {
              begin: async (name, given) => {
                const aimed = await aim(name, given)
                if (!aimed || docNow.current !== open) throw new Error('the document changed')
                return open.invoke<string>('strokeBegin', name, JSON.stringify(aimed))
              },
              push: (stroke, points) => open.invoke<Int32Array>('strokePush', stroke, points),
              cancel: (stroke) => open.invoke<Int32Array>('strokeCancel', stroke),
              repaint: (area) => (docNow.current === open ? viewport.patch(area) : Promise.resolve(false)),
            },
            command,
            params,
            failed,
          )
        },
        omitLayer: (layer, viewport, area) => {
          const open = docNow.current
          if (!open) return
          void open
            .invoke('omitLayer', layer ?? -1)
            .then(() => {
              if (docNow.current !== open) return
              return area ? viewport.patch(area) : viewport.render()
            })
            .catch(() => undefined)
        },
        paths,
        layerKind: (layer) => stateNow.current?.layers.find((item) => item.id === layer)?.kind ?? null,
        snap: snapper,
    }),
    // The getters read refs; what is passed by value is in the list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [cloneSource, query, runTool, toolGeneration, updateCrop, revisionNow, say, redraw, t, overlays.outline, aim, paths, snapper],
  )
  const transformOpen = transform.box != null
  // While Free Transform's box is open the canvas is the box's: asked on every press, so a box a
  // press opens (a Show Transform Controls handle) keeps that press's drag.
  const boxTool = useMemo(() => transformTool(env, () => void transformNow.current.commit()), [env])
  const canvasTool = useMemo(() => {
    const own0 = photoCanvasTool(tool, env)
    // The Move tool grabs guides (plan 2026-11-24, stage 7).
    const made = tool === 'Move' ? withGuides(own0, guideControl) : own0
    const needed = () => (paintsPixels(tool, optionsNow.current.gradientClassic) ? rasterizeNeeded(stateNow.current, maskTargetNow.current) : null)
    // Alt-click with the Clone Stamp or Healing Brush only sets the source: it paints nothing.
    const paints = (pointer: { alt: boolean; meta: boolean }) => paintsPixels(tool, optionsNow.current.gradientClassic) && !((tool === 'CloneStamp' || tool === 'Healing' || ALT_PICKS.has(tool)) && pointer.alt && !pointer.meta)
    const own = withRasterizePrompt(withPaintLease(made, paints, paintLease), tool, needed, (ask) => {
      if (!rasterizeOpen.current) setRasterize({ ...ask, generation: toolGeneration })
      rasterizeOpen.current = true
    })
    const boxed = () => transformNow.current.boxNow.current != null
    const toolNow: KitTool = {
      ...own,
      begin: (pointer, context) => (boxed() ? boxTool.begin(pointer, context) : own.begin(pointer, context)),
      hover: (pointer, context) => (boxed() ? boxTool.hover?.(pointer, context) : own.hover?.(pointer, context)),
      doubleClick: (pointer, context) => (boxed() ? boxTool.doubleClick?.(pointer, context) : own.doubleClick?.(pointer, context)),
      leave: (context) => {
        boxTool.leave?.(context)
        own.leave?.(context)
      },
    }
    return toolNow
  }, [env, paintLease, tool, toolGeneration, boxTool, guideControl])
  // Cmd held turns a press into the Move tool's (Photoshop's), except for tools whose Cmd is their own.
  const moveOptionsNow = useRef(optionsFor(optionsState, 'Move'))
  moveOptionsNow.current = optionsFor(optionsState, 'Move')
  const temporaryMove = useMemo(
    () => (CMD_IS_OWN.has(tool) || transformOpen ? null : withGuides(photoCanvasTool('Move', Object.create(env, { options: { get: () => moveOptionsNow.current } }) as PhotoToolEnv), guideControl)),
    [env, tool, transformOpen, guideControl],
  )
  // The Info panel reads the pointer on every move over the canvas, whatever the tool or the view is
  // doing; the brush outline follows it in the canvas's pixels.
  const trackedTool = useMemo(() => {
    const tracked = trackPointer(canvasTool, pointer.set)
    return {
      ...tracked,
      track: (at: Parameters<NonNullable<KitTool['track']>>[0], context: Parameters<NonNullable<KitTool['track']>>[1]) => {
        canvasPointer.set({ x: at.canvasX, y: at.canvasY })
        tracked.track?.(at, context)
      },
      leave: (context: Parameters<NonNullable<KitTool['leave']>>[0]) => {
        canvasPointer.set(null)
        tracked.leave?.(context)
      },
    }
  }, [canvasTool, pointer, canvasPointer])
  // The brush outline: the brush tools' cursor, unless a held key gives the press to something else.
  const brushLike = Boolean(toolFor(tool)?.brush) && !transformOpen
  const altOwn = keys.alt && !keys.meta && (ALT_PICKS.has(tool) || tool === 'CloneStamp' || tool === 'Healing')
  // Caps Lock gives the other of the two cursors Preferences › General chooses (outline or precise).
  const precise = (prefs.brushCursor === 'precise') !== keys.capsLock
  const outline = brushLike && !keys.space && !(keys.meta && temporaryMove) && !altOwn && !precise ? outlineDiameter(options.size, zoom) : null

  // A right click on the canvas opens the tool's menu there (plan 2026-11-24, stage 7), with a brush
  // tool the brush preset picker (stage 5); Cmd (Ctrl) held lists the layers under the pointer. The
  // shared canvas drops right presses: Photo listens itself, as for Space and the wheel.
  const brushLikeNow = useRef(brushLike)
  brushLikeNow.current = brushLike
  const [canvasMenu, setCanvasMenu] = useState<{ at: { x: number; y: number }; rows: CanvasMenuRow[]; point: { x: number; y: number } | null } | null>(null)
  const openCanvasMenuNow = useRef<(at: { x: number; y: number }, meta: boolean) => void>(() => undefined)
  useEffect(() => {
    const menu = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Element) || target.getAttribute('data-testid') !== 'kit-canvas-surface' || !workspace.current?.contains(target)) return
      event.preventDefault()
      event.stopPropagation()
      openCanvasMenuNow.current({ x: event.clientX, y: event.clientY }, event.metaKey || event.ctrlKey)
    }
    window.addEventListener('contextmenu', menu, true)
    return () => window.removeEventListener('contextmenu', menu, true)
  }, [])

  /** Makes `next` the tool; `PhotoToolRail` has checked that the host allows it. */
  const chooseTool = async (next: PhotoToolId) => {
    const navigation = ++textNavigation.current
    if (next !== tool) {
      // Choosing another tool commits text edited on the canvas, as Photoshop's does.
      if (typeSession.active) await typeSession.commit()
      // Visiting text options leaves the draft open but no longer focused, so switching tools
      // may not blur it. Finish with the old tool's options and keep focus on the chosen tool.
      const actions = textActions.current
      if (actions && !(await actions.commit(false))) return
      // Several clicks can share the pending submission; only the latest may navigate.
      if (navigation !== textNavigation.current) return
      pending.current?.cancel()
    }
    // Choosing another tool applies an open Free Transform, as Photoshop's does.
    if (transformNow.current.boxNow.current && !(await transformNow.current.commit().then(() => true, () => false))) return
    if (navigation !== textNavigation.current) return
    setTool(next)
    setToolCursor(null)
    // The Crop tool starts with a frame round the whole canvas.
    const size = stateNow.current
    updateCrop(next === 'Crop' && size ? { x: 0, y: 0, width: size.width, height: size.height, angle: 0, initial: true } : null, true)
  }
  const applyCrop = async () => {
    const frame = cropNow.current
    // The untouched first frame is the whole canvas: there is nothing to crop.
    if (!frame || frame.initial) return
    // Move focus before disabling Apply, while the command may still be waiting on the worker.
    // Do not focus again on completion: the user may have moved to another control meanwhile.
    workspace.current?.focus()
    updateCrop(null, true)
    if (await run('image.crop', { ...cropParams(frame), ...toolParams('Crop', options) })) {
      canvas.current?.fit()
    }
  }
  applyCropNow.current = () => void applyCrop()
  const cancelCrop = () => {
    workspace.current?.focus()
    updateCrop(null, true)
  }
  /** Edit › Free Transform (Cmd+T). */
  const startTransform = () => {
    if (!transformNow.current.start()) return
    workspace.current?.focus({ preventScroll: true })
  }
  // View's zoom rows, as upstream's (`menus.rs`, `view_cmds.rs`); Print Size assumes a 72 ppi screen.
  const viewCommand = (command: string) => {
    const handle = canvas.current
    const now = handle?.view()?.zoom ?? 1
    if (!handle) return
    if (command === 'kit:view.zoomIn') handle.zoomTo(zoomStep(now, 1))
    else if (command === 'kit:view.zoomOut') handle.zoomTo(zoomStep(now, -1))
    else if (command === 'kit:view.fitOnScreen') handle.fit()
    else if (command === 'kit:view.fitLayersOnScreen') handle.show(state?.activeBounds ?? { x0: 0, y0: 0, x1: state?.width ?? 1, y1: state?.height ?? 1 })
    else if (command === 'kit:view.actualPixels') handle.zoomTo(1)
    else if (command === 'kit:view.twoHundredPercent') handle.zoomTo(2)
    else if (command === 'kit:view.printSize') handle.zoomTo(72 / Math.max(1, state?.resolution ?? 72))
  }
  // Kition's own menu rows: Help's dialogs, View's zoom rows, and the Window and Type › Panels rows
  // (a panel row may bring its tab forward, expanding its group).
  const kitCommand = (command: string) => {
    if (command === 'kit:help.about') setHelp('about')
    else if (command === 'kit:help.shortcuts') setHelp('shortcuts')
    else if (command.startsWith('kit:view.')) viewCommand(command)
    else {
      const next = windowCommand(windows, command, dockTabs)
      setWindows(next.state)
      if (next.reveal) dock.current?.showTab(next.reveal)
    }
  }
  // The menus' rows run the latest `kitCommand`; the tabs are built again only when the windows change.
  const kitCommandNow = useRef(kitCommand)
  kitCommandNow.current = kitCommand
  const runKitCommand = useCallback((command: string) => kitCommandNow.current(command), [])
  // The dialog host (`PhotoDialogs`, inside the editor's provider) answers through `dialogApi`; the
  // menus, built out here, reach it through `dialogs`. A "…" row opens its dialog instead of running.
  const dialogApi = useRef<Dialogs | null>(null)
  const runNow = useRef(run)
  runNow.current = run
  // Files in and out (plan 2026-11-24, stage 6): Place Embedded picks through the host, Copy also
  // fills the system clipboard, Paste takes an image from it (`io/PhotoFileIo.tsx`).
  const fileIo = useRef<FileIo | null>(null)
  // View's rows the editor answers (Rulers, Show, Snap, Snap To, Lock Guides: plan 2026-11-24, stage 7).
  const viewRowNow = useRef((command: string) => setViewSettings(viewRow(viewSettingsNow.current, command)))
  viewRowNow.current = (command: string) => setViewSettings(viewRow(viewSettingsNow.current, command))
  const dialogs = useMemo<Dialogs>(
    () => ({
      invoke: (command) => {
        if (VIEW_SETTING_ROWS.has(command)) return viewRowNow.current(command)
        const io = fileIo.current
        if (io && command === 'file.placeEmbedded') return void io.place()
        if (io && (command === 'edit.copy' || command === 'edit.copyMerged')) return io.copy(command)
        if (io && command === 'edit.paste') return io.paste()
        return dialogApi.current ? dialogApi.current.invoke(command) : void runNow.current(command)
      },
      open: (command, values) => dialogApi.current?.open(command, values) ?? false,
      asks: (command) => dialogApi.current?.asks(command) ?? false,
      newDocument: () => dialogApi.current?.newDocument(),
    }),
    [],
  )
  // File › New… (the engine's row is the host's): Kition's own action, at the top of the File menu.
  // Edit › Free Transform (Cmd+T): Kition's own row at the top of Edit (the engine's transform takes the box the canvas draws).
  const startTransformNow = useRef(startTransform)
  startTransformNow.current = startTransform
  // File's own rows (plan 2026-11-24, stage 6): what the host can do, and nothing it cannot.
  const saveNow = useRef<() => unknown>(() => undefined)
  const abilities = useMemo(() => fileAbilities(host), [host])
  const fileActions = useMemo(
    () => ({
      File: [
        { label: t('dialog.newDocumentMenu'), onSelect: dialogs.newDocument, testId: 'photo-file-new-document' },
        ...(abilities.open ? [{ label: t('file.open'), onSelect: () => void fileIo.current?.open(), testId: 'photo-file-open' }] : []),
        ...(host.readOnly ? [] : [{ label: t('file.save'), onSelect: () => void saveNow.current(), testId: 'photo-file-save' }]),
        ...(abilities.saveAs ? [{ label: t('file.saveAs'), onSelect: () => void fileIo.current?.saveAs(), testId: 'photo-file-save-as' }] : []),
        ...(abilities.export
          ? [
              { label: t('file.exportAs'), onSelect: () => fileIo.current?.exportAs(), testId: 'photo-file-export-as' },
              { label: t('file.quickExport'), onSelect: () => void fileIo.current?.quickExport(), testId: 'photo-file-quick-export' },
            ]
          : []),
      ],
      Edit: [{ label: phrase('Free Transform'), onSelect: () => startTransformNow.current(), testId: 'photo-edit-free-transform' }],
      // Window › Brush Settings (F5) and Brushes: Kition's own floating windows (plan 2026-11-24, stage 5).
      Window: [
        { label: phrase('Brush Settings'), onSelect: () => toggleFloating('brushSettings'), testId: 'photo-window-brush-settings' },
        { label: phrase('Brushes'), onSelect: () => toggleFloating('brushes'), testId: 'photo-window-brushes' },
      ],
    }),
    [t, dialogs, phrase, toggleFloating, abilities, host],
  )
  const viewCheck = useCallback((command: string) => viewChecked(viewSettings, command), [viewSettings])
  const menuTabs = useMemo(() => photoMenuTabs({ windows, dock: dockTabs, onKitCommand: runKitCommand, onCommand: dialogs.invoke, before: fileActions, phrase, viewChecked: viewCheck }), [windows, dockTabs, runKitCommand, dialogs, fileActions, phrase, viewCheck])
  // Home's tool buttons choose as the rail does (the latest `chooseTool`); the tabs are built again when the tool changes, for its pressed state.
  const chooseToolNow = useRef(chooseTool)
  chooseToolNow.current = chooseTool
  const chooseFromHome = useCallback((id: PhotoToolId) => void chooseToolNow.current(id), [])
  const tabs = useMemo<FrameTab[]>(() => [{ id: 'home', label: t('homeTab'), ribbon: <HomeTab tool={tool} onTool={chooseFromHome} /> }, ...menuTabs], [t, tool, chooseFromHome, menuTabs])

  const cancelText = (focusEditor: boolean) => {
    focusAfterText.current = focusEditor
    setTextDirty(false)
    setTextDraft(null)
  }
  // New text takes the Type tool's options (with its anti-aliasing, in one step) and the foreground colour.
  // Enter (`goOn`) goes on editing it on the canvas, on a new line, in the same history step.
  const placeText = (text: string, focusEditor: boolean, goOn = false) => {
    const draft = textDraft
    if (!draft) return Promise.resolve(false)
    focusAfterText.current = focusEditor
    setTextPending(true)
    const generation = toolGenerationNow.current
    const navigation = textNavigation.current
    const key = sessionKey()
    const font = pendingTypeFont(doc)
    const create = (choice: Partial<ToolOptions> = {}) => generation === toolGenerationNow.current
      ? run('text.add', { x: Math.round(draft.x), y: Math.round(draft.y), text, color: fg, orientation: draft.orientation, ...toolParams('Type', { ...options, ...choice }), coalesce: key })
      : Promise.resolve(null)
    // A font selection is an editing intent even before metadata answers. Capture its result
    // for this draft; later option changes still belong to the next draft (UI38).
    return (font ? font.then((choice) => choice ? create(choice) : null) : create()).then((result) => {
      if (result !== null) {
        setTextDirty(false)
        setTextDraft(null)
        if (goOn) void continueText(result, key, navigation, revisionNow())
      }
      // Restore typing only if focus is still in the workspace; preserve a later field choice.
      else focusText.current = document.activeElement === workspace.current
      setTextPending(false)
      return result !== null
    })
  }

  // Every editor Save entry point must finish a draft, including one left open in text options.
  // Repeated requests for the same content share a write; newer edits request a follow-up.
  saveNow.current = () => saveWithText()
  const saveWithText = () => {
    if (textSave.current) {
      const target = textSaveTarget.current
      if (target && (target.draft !== textDraftId.current || (target.revision !== null && (target.revision !== doc?.info.revision || target.command !== commandSequence.current)))) textSaveAgain.current = true
      return textSave.current
    }
    const saving = (async () => {
      do {
        textSaveAgain.current = false
        const target = { draft: textDraftId.current, revision: null as number | null, command: commandSequence.current }
        textSaveTarget.current = target
        if (typeSession.active) await typeSession.commit()
        const actions = textActions.current
        if (actions && !(await actions.commit())) return
        const fonts = await Promise.all(pendingFontChoices(doc))
        if (fonts.some((choice) => choice === null)) return
        // A requested Undo/Redo may still be waiting on the worker and have no new revision
        // yet. Include commands already submitted before taking the save's content boundary.
        await Promise.allSettled([...pendingCommands.current])
        // A draft's own commit changes revision. Compare later requests with its committed
        // revision, so repeating Save during that commit does not schedule a duplicate write.
        target.revision = doc?.info.revision ?? null
        target.command = commandSequence.current
        await editor.save()
      } while (textSaveAgain.current)
    })().finally(() => {
      textSave.current = null
      textSaveTarget.current = null
    })
    textSave.current = saving
    return saving
  }

  /** The layer id a `text.add` answered (its JSON), or null. */
  const createdLayer = (result: string | null): number | null => {
    try {
      const layer = result ? (JSON.parse(result) as { layer?: unknown }).layer : null
      return typeof layer === 'number' ? layer : null
    } catch {
      return null
    }
  }
  /** After Enter on new text: edit it on the canvas, the caret on a new line, if nothing came between. */
  const continueText = async (result: string | null, key: string, navigation: number, revision: number) => {
    const layer = createdLayer(result)
    if (layer == null) return
    // Let the field go and the keyboard come back to the canvas first.
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    if (navigation !== textNavigation.current || revisionNow() !== revision || document.activeElement !== workspace.current) return
    await typeSession.start({ layer, created: true, key, pendingBreak: true })
  }
  /** Edits type layer `layer` with `select` selected (its caret when empty); the layer becomes the active one first. */
  const startEditing = async (layer: number, select: TextRange) => {
    // A fully locked layer refuses every edit: say so once instead of at every key.
    if (stateNow.current?.layers.find((item) => item.id === layer)?.locks.all) {
      say('type.locked')
      return false
    }
    if (stateNow.current?.activeLayer !== layer && (await run('layer.select', { layer, mode: 'replace' })) === null) return false
    return typeSession.start({ layer, select })
  }
  /** Chooses the Type tool for a layer's orientation, then edits it (the Move tool's double click, a Layers thumbnail's). */
  const editLayerText = async (layer: number, select: (length: number) => TextRange) => {
    if (editor.host.readOnly) return
    const info = await query<{ text: string; orientation: string }>('text.info', { layer })
    if (!info || typeof info.text !== 'string') return
    const next: PhotoToolId = info.orientation === 'Vertical' ? 'VerticalType' : 'Type'
    if (next !== tool) await chooseToolNow.current(next)
    await startEditing(layer, select(Array.from(info.text).length))
  }
  editTextAtNow.current = async (pointer) => {
    const hit = await query<{ layer: number; index: number }>('text.hitTest', { x: pointer.x, y: pointer.y })
    if (typeof hit?.layer === 'number') await editLayerText(hit.layer, () => [hit.index, hit.index])
  }
  const typeEditingApi = useMemo<TypeEditing>(() => ({ target: typeSession.target, editLayer: (layer) => void editLayerTextNow.current(layer, (length) => [0, length]) }), [typeSession])
  const editLayerTextNow = useRef(editLayerText)
  editLayerTextNow.current = editLayerText
  /** A new paragraph box dragged out: an empty paragraph layer, edited at once (removed if left empty). */
  const placeParagraph = async (box: [number, number, number, number], orientation: 'horizontal' | 'vertical', navigation: number) => {
    const key = sessionKey()
    const result = await run('text.add', { box, text: '', color: colorsNow.current.foreground, orientation, ...toolParams('Type', optionsNow.current), coalesce: key })
    const layer = createdLayer(result)
    if (layer == null || navigation !== textNavigation.current) return
    await typeSession.start({ layer, created: true, key, select: [0, 0] })
  }
  /** A Type tool press with no text edited: on type it edits it; elsewhere a click places point text, a drag a paragraph box. */
  const freshTypePress = (pointer: KitPointer, orientation: 'horizontal' | 'vertical'): ReturnType<KitTool['begin']> => {
    // The old draft can be unfocused after visiting its options. Finish it before replacing it; a
    // rejected command keeps the old draft available for retry. Only the latest press may go on.
    const navigation = ++textNavigation.current
    const draftActions = textActions.current
    const finished = Promise.all([draftActions ? draftActions.commit(false) : Promise.resolve(true), typeSession.settled()]).then(([committed]) => committed)
    const from = { x: pointer.canvasX, y: pointer.canvasY }
    // Counted now: a double click's second press can come before the first one's edit has started.
    const clicks = typeSession.countPress(from)
    let last = pointer
    let moved = false
    let onType = false
    let release: (outcome: 'cancel' | [number, number, number, number] | null) => void = () => undefined
    const released = new Promise<'cancel' | [number, number, number, number] | null>((resolve) => {
      release = resolve
    })
    void (async () => {
      if (!(await finished) || navigation !== textNavigation.current) return
      // A Shift-press makes new text even over type (Photoshop's way to start text on text).
      const hit = !pointer.shift && !editor.host.readOnly ? await query<{ layer: number; index: number }>('text.hitTest', { x: pointer.x, y: pointer.y }) : null
      if (navigation !== textNavigation.current) return
      if (typeof hit?.layer === 'number') {
        onType = true
        if (!(await startEditing(hit.layer, [hit.index, hit.index]))) return
        typeSession.press(pointer, from, false, clicks)
        if (moved) typeSession.dragTo(last)
        return
      }
      const outcome = await released
      if (outcome === 'cancel' || navigation !== textNavigation.current) return
      if (outcome) return void placeParagraph(outcome, orientation, navigation)
      focusText.current = true
      setTextDirty(false)
      setTextDraft({ id: ++textDraftId.current, x: pointer.x, y: pointer.y, left: pointer.canvasX, top: pointer.canvasY, orientation })
    })()
    return {
      move: (at, { overlay }) => {
        last = at
        moved = moved || Math.hypot(at.canvasX - from.x, at.canvasY - from.y) >= 3
        if (onType) {
          if (typeSession.active && moved) typeSession.dragTo(at)
          return
        }
        if (!isBoxDrag(from, { x: at.canvasX, y: at.canvasY })) return overlay.clear()
        // The paragraph box being dragged out.
        overlay.draw((context, toScreen) => {
          const a = toScreen(pointer.x, pointer.y)
          const b = toScreen(at.x, at.y)
          context.beginPath()
          context.rect(Math.min(a.x, b.x) + 0.5, Math.min(a.y, b.y) + 0.5, Math.abs(b.x - a.x), Math.abs(b.y - a.y))
          dashed(context)
        })
      },
      end: (at, { overlay }) => {
        last = at
        overlay.clear()
        release(!onType && isBoxDrag(from, { x: at.canvasX, y: at.canvasY }) ? dragBox(pointer, at) : null)
        if (onType && typeSession.active) {
          if (moved) typeSession.dragTo(at)
          typeSession.focusInput()
        }
      },
      cancel: ({ overlay }) => {
        overlay.clear()
        release('cancel')
      },
    }
  }
  typePressNow.current = (pointer, orientation, { viewport }) => {
    if (typeSession.active) {
      const screen = { x: pointer.canvasX, y: pointer.canvasY }
      const toScreen = (x: number, y: number) => viewport.toScreen(x, y)
      const handle = typeSession.handleAt(screen, toScreen)
      if (handle) {
        // A paragraph box's handle: the box follows the drag, its text wrapping as it goes.
        return {
          move: (at) => typeSession.resize(handle, at),
          end: (at) => {
            typeSession.resize(handle, at)
            typeSession.focusInput()
          },
        }
      }
      const zoom = canvas.current?.view()?.zoom ?? 1
      if (typeSession.contains(pointer, 4 / zoom)) {
        // On the edited text: the caret goes there (two clicks a word, three a paragraph), a drag selects.
        typeSession.press(pointer, screen, pointer.shift)
        let moved = false
        return {
          move: (at) => {
            moved = moved || Math.hypot(at.canvasX - screen.x, at.canvasY - screen.y) >= 3
            if (moved) typeSession.dragTo(at)
          },
          end: (at) => {
            if (moved) typeSession.dragTo(at)
            typeSession.focusInput()
          },
        }
      }
      // Elsewhere the press commits the edit, then does what a press does with nothing edited.
      void typeSession.commit()
    }
    return freshTypePress(pointer, orientation)
  }
  typeHoverNow.current = (pointer, { viewport }) => {
    const handle = typeSession.active ? typeSession.handleAt({ x: pointer.canvasX, y: pointer.canvasY }, (x, y) => viewport.toScreen(x, y)) : null
    setToolCursor(handle ? (handle === 'n' || handle === 's' ? 'ns-resize' : handle === 'e' || handle === 'w' ? 'ew-resize' : handle === 'nw' || handle === 'se' ? 'nwse-resize' : 'nesw-resize') : null)
  }
  /** Undo, Redo and Save from the keys of text being edited: the edit is committed first. */
  const typeAction = async (kind: 'undo' | 'redo' | 'save') => {
    await typeSession.commit()
    workspace.current?.focus({ preventScroll: true })
    if (kind === 'save') await saveWithText()
    else await run(kind === 'undo' ? 'edit.undo' : 'edit.redo', {})
  }

  /** The document point under page point (`clientX`, `clientY`), or null before the canvas is laid out. */
  const documentAt = (clientX: number, clientY: number) => {
    const surface = workspace.current?.querySelector('[data-testid="kit-canvas-surface"]')
    const shownView = canvas.current?.view()
    if (!surface || !shownView) return null
    const box = surface.getBoundingClientRect()
    return { x: shownView.x + (clientX - box.left) / (shownView.zoom || 1), y: shownView.y + (clientY - box.top) / (shownView.zoom || 1), zoom: shownView.zoom || 1 }
  }
  // The canvas's menu (plan 2026-11-24, stage 7): read what is under the pointer, then show the tool's rows.
  const menuSerial = useRef(0)
  // Read when the menu opens and when a row runs: this render's paths and tools.
  const pathsNow = {
    current: {
    target: async () => {
      const view = paths.get()
      return view.shape ? { kind: 'shape' as const } : view.work ? { kind: 'work' as const } : null
    },
    partAt: (_target: { kind: 'work' | 'shape' }, p: { x: number; y: number }, zoom: number): 'anchor' | 'segment' | null => {
      const part = partAtPoint(paths.get(), [p.x, p.y], 6 / zoom)
      return part?.anchor ? 'anchor' : part?.segment ? 'segment' : null
    },
    editPoint: (kind: 'addAnchor' | 'deleteAnchor', p: { x: number; y: number }) => editPointAt(env, kind, [p.x, p.y], canvas.current?.view()?.zoom ?? 1),
    },
  }
  openCanvasMenuNow.current = (at, meta) => {
    if (brushLikeNow.current && !meta) return setBrushPicker(at)
    const serial = ++menuSerial.current
    const point = documentAt(at.x, at.y)
    const now = stateNow.current
    void (async () => {
      const inside = point != null && now != null && point.x >= 0 && point.y >= 0 && point.x < now.width && point.y < now.height
      const picked = (meta || tool === 'Move') && inside ? await query<{ layers?: { layer: number; name: string }[] }>('layer.pickAt', { x: Math.floor(point.x), y: Math.floor(point.y), list: true, select: false }) : null
      const pathTool = PATH_TOOLS.has(tool) && !meta
      const target = pathTool ? await pathsNow.current.target() : null
      const latest = stateNow.current
      const active = latest?.layers.find((layer) => layer.id === latest.activeLayer)
      const context: CanvasMenuContext = {
        tool,
        layersOnly: meta,
        hasSelection: latest?.hasSelection ?? false,
        layer: active ? { kind: active.kind, background: active.background } : null,
        layersHere: (picked?.layers ?? []).map((layer) => ({ id: layer.layer, name: layer.name })),
        activeLayer: latest?.activeLayer ?? null,
        typeEditing: typeSession.active || textDraft != null,
        path: target?.kind ?? null,
        pathPart: pathTool && target && point ? pathsNow.current.partAt(target, point, point.zoom) : null,
        penOpen: tool === 'Pen' && pending.current?.tool === 'Pen',
        cropping: cropNow.current != null && !cropNow.current.initial,
        sampleSize: optionsFor(optionsState, 'Eyedropper').sampleSize,
      }
      const rows = canvasMenuRows(context)
      if (serial !== menuSerial.current || !rows.length) return
      setCanvasMenu({ at, rows, point })
    })()
  }
  /** A choice in the canvas's menu: a command (a "…" one through its dialog), a layer, or the editor's own action. */
  const chooseFromCanvasMenu = (row: Extract<CanvasMenuRow, { kind: 'item' }>, point: { x: number; y: number } | null) => {
    workspace.current?.focus({ preventScroll: true })
    const action = row.action
    if ('layer' in action) return void run('layer.select', { layer: action.layer, mode: 'replace' })
    if ('command' in action) return action.params ? void run(action.command, action.params) : dialogs.invoke(action.command)
    if (action.own === 'freeTransform') return startTransform()
    if (action.own === 'editType') {
      const layer = stateNow.current?.activeLayer
      return layer != null ? void editLayerTextNow.current(layer, (length) => [0, length]) : undefined
    }
    if (action.own === 'commitType') return void (typeSession.active ? typeSession.commit() : textActions.current?.commit())
    if (action.own === 'cancelType') return void (typeSession.active ? typeSession.cancel() : textActions.current?.cancel())
    if (action.own === 'cropCommit') return void applyCrop()
    if (action.own === 'cropCancel') return cancelCrop()
    if (action.own === 'sampleSize' && action.value) return setToolOptions('Eyedropper', { sampleSize: action.value })
    if (action.own === 'view' && action.value) return viewCommand(action.value)
    if (action.own === 'closePath') return pending.current?.tool === 'Pen' ? (pending.current as PendingClicks & { close?: () => void }).close?.() : undefined
    if ((action.own === 'deleteAnchor' || action.own === 'addAnchor') && point) return void pathsNow.current.editPoint(action.own, point)
  }
  const chooseFromCanvasMenuNow = useRef(chooseFromCanvasMenu)
  chooseFromCanvasMenuNow.current = chooseFromCanvasMenu
  /** The menu key or Shift+F10: the canvas's menu at the pointer (the canvas's centre when the pointer is elsewhere). */
  const menuFromKeys = () => {
    const surface = workspace.current?.querySelector('[data-testid="kit-canvas-surface"]')
    if (!surface) return
    const box = surface.getBoundingClientRect()
    const over = canvasPointer.get()
    openCanvasMenuNow.current(over ? { x: box.left + over.x, y: box.top + over.y } : { x: box.left + box.width / 2, y: box.top + box.height / 2 }, false)
  }

  // Arrow keys nudge (plan 2026-11-24, stage 2): Free Transform's box, the crop frame, the active
  // layer (its selected pixels with a selection) with the Move tool or Cmd held, and the selection's
  // outline with a selection tool; 1 screen pixel, 10 document pixels with Shift. Bundled presses are one step.
  const pathNudge = useRef({ at: 0, key: '' })
  const nudge = (event: KeyboardEvent<HTMLDivElement>, command: boolean): boolean => {
    const arrow = ARROWS[event.key.toLowerCase()]
    if (!arrow || (event.target instanceof Element && event.target.closest(OWN_KEYS))) return false
    const step = nudgeStep(canvas.current?.view()?.zoom ?? 1, event.shiftKey)
    const [dx, dy] = [arrow[0] * step, arrow[1] * step]
    const box = transformNow.current.boxNow.current
    if (box) {
      transformNow.current.update(box.quad.map((p) => ({ x: p.x + dx, y: p.y + dy })) as typeof box.quad, { x: box.reference.x + dx, y: box.reference.y + dy }, true)
      return true
    }
    const frame = cropNow.current
    if (frame && !frame.initial) {
      updateCrop({ ...frame, x: frame.x + dx, y: frame.y + dy }, true)
      return true
    }
    // A path's selected anchors (the path tools), one step for presses in a row.
    if (PATH_EDIT_TOOLS.has(tool) && paths.get().selected?.anchors.length) {
      const at = Date.now()
      if (at - pathNudge.current.at > 1000) pathNudge.current.key = `photo-path-nudge-${at}`
      pathNudge.current.at = at
      void nudgeAnchors(env, dx, dy, pathNudge.current.key)
      return true
    }
    const now = stateNow.current
    const active = now?.layers.find((layer) => layer.id === now.activeLayer)
    if (tool === 'Move' || (command && temporaryMove)) {
      if (!active) return true
      if (now?.hasSelection && active.kind === 'Pixel') {
        if ((active.locks.position || active.locks.all) && !active.background) say('move.locked')
        else nudges.nudge('float', 'select.float', dx, dy, { copy: false })
      } else if (active.background || active.locks.position || active.locks.all) say('move.locked')
      else nudges.nudge('layer', 'layer.translate', dx, dy)
      return true
    }
    if (OUTLINE_TOOLS.has(tool) && now?.hasSelection && !pending.current && !command) {
      nudges.nudge('outline', 'select.transformSelection', dx, dy)
      return true
    }
    return false
  }

  // Photoshop's keys (kit-editor's canvas drops the drag under way on Escape): Enter and Escape finish
  // or drop Free Transform, a crop or a several-click tool (Pen, Polygonal Lasso; Backspace takes its
  // last point back), Cmd+T opens Free Transform, arrows nudge, [ and ] size the brush (Shift: its
  // hardness), number keys set its opacity (Shift: flow), X swaps the colors, D resets them, letters
  // choose tools. An Escape with none of these to drop is not taken, so in Focus it leaves Focus
  // (KitPointer 4.4); nor is one on the crop tool's untouched first frame.
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const command = event.metaKey || event.ctrlKey
    const key = event.key.toLowerCase()
    // Text being edited while the keyboard is elsewhere in the editor (after the options bar, say):
    // Escape cancels it and Cmd+Enter commits it, as in its own field.
    if (typeSession.active && !(event.target instanceof Element && event.target.closest(OWN_KEYS))) {
      if (key === 'escape') {
        void typeSession.cancel()
        return true
      }
      if (key === 'enter' && (command || event.code === 'NumpadEnter')) {
        void typeSession.commit()
        return true
      }
    }
    // The menu key and Shift+F10: the canvas's menu (plan 2026-11-24, stage 7).
    if ((event.key === 'ContextMenu' || (event.shiftKey && !command && !event.altKey && event.key === 'F10')) && !(event.target instanceof Element && event.target.closest('input, select, [role="menu"], [role="menuitem"]'))) {
      menuFromKeys()
      return true
    }
    // View's own rows' keys: Cmd+R Rulers, Cmd+; Guides, Cmd+' Grid, Cmd+H Extras, Cmd+Shift+; Snap, Cmd+Alt+; Lock Guides.
    const viewKey = VIEW_KEY_ROWS.find((row) => shortcutMatches(row.shortcut!, event, undefined, true))
    if (viewKey) {
      viewRowNow.current(viewKey.command)
      return true
    }
    const box = transformNow.current.boxNow.current
    const frame = cropNow.current
    if (box && key === 'enter') void transformNow.current.commit()
    else if (box && key === 'escape') void transformNow.current.cancel()
    else if (command && !event.altKey && !event.shiftKey && key === 't') startTransform()
    else if (menuKeys.current?.(event)) return true
    else if (frame && !frame.initial && key === 'enter') void applyCrop()
    else if (frame && !frame.initial && key === 'escape') cancelCrop()
    else if (pending.current && key === 'enter') pending.current.finish()
    // Escape finishes the Pen's path as Enter does (Photoshop's); it drops the other tools' clicks.
    else if (pending.current && key === 'escape') (pending.current.tool === 'Pen' ? pending.current.finish() : pending.current.cancel())
    else if (pending.current?.back && (key === 'backspace' || key === 'delete')) pending.current.back()
    // Delete on a path's selected anchors takes them out; Escape lets them go (plan 2026-11-24, stage 7).
    else if (PATH_EDIT_TOOLS.has(tool) && paths.get().selected?.anchors.length && (key === 'backspace' || key === 'delete') && !command && !(event.target instanceof Element && event.target.closest(OWN_KEYS))) void deleteSelectedAnchors(env)
    else if (PATH_EDIT_TOOLS.has(tool) && paths.get().selected?.anchors.length && key === 'escape' && !command) {
      paths.set({ ...paths.get(), selected: null })
    }
    else if (nudge(event, command)) return true
    else if (command || event.altKey) return false
    else if (event.code === 'BracketLeft' || event.code === 'BracketRight') stepBrush(event.code === 'BracketRight' ? 1 : -1, event.shiftKey)
    else if (/^Digit[0-9]$/.test(event.code) && !(event.target instanceof Element && event.target.closest(OWN_KEYS))) typeDigit(Number(event.code.slice(5)), event.shiftKey)
    else if (event.key === 'F5') toggleFloating('brushSettings')
    else if (key === 'x') colors.swap()
    else if (key === 'd') colors.reset()
    else return toolKeys.current?.(event) ?? false
    return true
  }

  const railTool = toolFor(tool)
  // The brush panels change the shared brush's options (size, hardness, pressure) as the bar does.
  const brushOptions = useMemo<BrushOptions>(() => ({ size: options.size, hardness: options.hardness, set: (patch) => setToolOptions(tool, patch) }), [options.size, options.hardness, setToolOptions, tool])
  const smoothing = useMemo(() => ((SMOOTHING_TOOLS as readonly string[]).includes(tool) ? { value: options.smoothing, set: (value: number) => setToolOptions(tool, { smoothing: value }) } : null), [tool, options.smoothing, setToolOptions])
  const brushSettingsBody = useCallback(() => <BrushSettingsPanel options={brushOptions} smoothing={smoothing} />, [brushOptions, smoothing])
  const brushesBody = useCallback(() => <BrushPresets options={brushOptions} testId="photo-brushes" />, [brushOptions])
  const floatingPanels = useMemo<FloatingPanel[]>(
    () =>
      floating.flatMap((id): FloatingPanel[] =>
        id === 'brushSettings' ? [{ id, title: phrase('Brush Settings'), render: brushSettingsBody }] : id === 'brushes' ? [{ id, title: phrase('Brushes'), render: brushesBody }] : [],
      ),
    [floating, phrase, brushSettingsBody, brushesBody],
  )
  const closeFloating = useCallback((id: string) => setFloating((open) => open.filter((item) => item !== id)), [])
  // View's own rows (Rulers, Show, Snap…) have no engine command: the editor answers them, so they are live.
  const editorEnabled = editor.enabled
  const photoEnabled = useCallback((command: string) => VIEW_SETTING_ROWS.has(command) || editorEnabled(command), [editorEnabled])
  const surfaceBox = useCallback(() => workspace.current?.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect() ?? null, [])
  const dropGuide = useCallback((orientation: 'horizontal' | 'vertical', position: number) => void typeRun.current('view.newGuide', { orientation, position }), [])
  const rulerLabel = useCallback((side: 'top' | 'left') => t(side === 'top' ? 'ruler.top' : 'ruler.left'), [t])

  return (
    <KitEditorProvider editor={{ ...editor, enabled: photoEnabled, save: saveWithText, session: { ...session, dirty: session.dirty || textDirty } }}>
      <TypeEditingContext.Provider value={typeEditingApi}>
      <DialogsProvider value={dialogs}>
      <ColorPickerProvider sample={sampleAt}>
      <GradientEditorProvider colors={colors}>
        <EditorFrame
          tabs={tabs}
          defaultTab="home"
          options={
            windows.options ? (
              <OptionsBar
                tool={tool}
                o={options}
                set={(patch) => setToolOptions(tool, patch)}
                foreground={fg}
                background={bg}
                setForeground={(value) => colors.set({ foreground: value })}
                layer={activeLayer ? { id: activeLayer.id, kind: activeLayer.kind } : null}
                crop={{ proposed: crop != null && !crop.initial, commit: () => void applyCrop(), cancel: cancelCrop }}
                transform={transform.fields ? { fields: transform.fields, set: transform.setFields, commit: () => void transform.commit(), cancel: () => void transform.cancel() } : null}
                text={{
                  editing: textDraft != null || typeEditing,
                  submitting: textPending,
                  layer: typeTarget?.layer ?? null,
                  commit: () => (typeSession.active ? void typeSession.commit() : textActions.current?.commit()),
                  cancel: () => (typeSession.active ? void typeSession.cancel() : textActions.current?.cancel()),
                }}
                cloneSourceSet={cloneSource != null}
                view={{ fit: () => canvas.current?.fit(), actual: () => canvas.current?.zoomTo(1) }}
                brushPicker={setBrushPicker}
              />
            ) : undefined
          }
          canvas={canvas}
          statusBar={windows.status}
          hiddenCommands={isHiddenCommand}
          onKeyDown={keyDown}
          status={state ? <PhotoStatus info={state} zoom={zoom} canvas={canvas} tool={tool} onZoomDone={() => workspace.current?.focus({ preventScroll: true })} /> : null}
          statusEnd={
            <span className="tabular-nums" data-testid="photo-web-stats">
              {[session.openMs != null ? t('stats.open', { ms: session.openMs }) : null, stats ? t('stats.render', { ms: stats.renderMs }) : null].filter(Boolean).join(' · ')}
            </span>
          }
          focusTool={railTool ? { label: phrase(railTool.label), icon: railTool.icon } : null}
          rail={
            <PhotoToolRail active={tool} onChoose={chooseTool} shown={windows.toolbar} keys={toolKeys} onView={(fit) => (fit ? canvas.current?.fit() : canvas.current?.zoomTo(1))}>
              <KitElement id="control:colors">
                <div className="photo-colors" title={t('colors')}>
                  {/* Each chip opens the Color Picker; the colour follows the picker as it moves (plan 2026-11-24, stage 5). */}
                  <ColorWell className="photo-color photo-color-background" value={bg} label={phrase('Set background color')} title={t('color.title.background')} onChange={(value) => colors.set({ background: value })} onLive={(value) => colors.set({ background: value })} testId="photo-background" />
                  <ColorWell className="photo-color photo-color-foreground" value={fg} label={phrase('Set foreground color')} title={t('color.title.foreground')} onChange={(value) => colors.set({ foreground: value })} onLive={(value) => colors.set({ foreground: value })} testId="photo-foreground" />
                  <span className="photo-color-actions">
                    <IconButton icon={ArrowLeftRight} label={t('swapColors')} onClick={colors.swap} data-testid="photo-colors-swap" />
                    <IconButton icon={RotateCcw} label={t('defaultColors')} onClick={colors.reset} data-testid="photo-colors-reset" />
                  </span>
                </div>
              </KitElement>
              {/* Upstream's Quick Mask button under the chips (Q, as Select › Edit in Quick Mask Mode). */}
              <KitElement id="control:quickMask">
                <span className="photo-rail-modes">
                  <IconButton
                    icon={SquareDashed}
                    label={phrase(quickMask ? 'Edit in Standard Mode  (Q)' : 'Edit in Quick Mask Mode  (Q)')}
                    pressed={quickMask}
                    disabled={!editor.enabled('select.editInQuickMaskMode')}
                    onClick={() => void run('select.editInQuickMaskMode', {})}
                    data-testid="photo-quick-mask"
                  />
                </span>
              </KitElement>
            </PhotoToolRail>
          }
          panels={
            state ? (
              <PhotoDock
                state={{
                  info: state,
                  maskTarget,
                  onMaskTarget: setMaskTarget,
                  colors,
                  typeOptions: optionsFor(optionsState, 'Type'),
                  setTypeOptions: (patch) => setToolOptions('Type', patch),
                  typing: textDraft != null,
                  pointer,
                  view,
                  canvas,
                }}
                windows={windows}
                dock={dock}
              />
            ) : null
          }
        >
          <PhotoMenuKeys onKitCommand={runKitCommand} keys={menuKeys} dialogs={dialogs} />
          <PhotoDialogs api={dialogApi} info={state} colors={colors} prefs={{ value: prefs, save: savePrefs }} onPlace={() => void fileIo.current?.place()} />
          <PhotoFileIo api={fileIo} workspace={workspace} canvas={canvas} size={state ? { width: state.width, height: state.height } : null} busy={busy} editing={() => typeSession.active || textDraft != null} />
          <GradientEditorOutlet />
          <ColorPickerOutlet />
          <div className="kui-row photo-workspace" ref={workspace} tabIndex={-1} data-testid="photo-workspace" data-rulers={viewSettings.rulers ? '' : undefined} data-guides={JSON.stringify(guides)}>
            {viewSettings.rulers ? <Rulers view={view} pointer={canvasPointer} surface={surfaceBox} onDraft={setGuideDraft} onDrop={dropGuide} label={rulerLabel} /> : null}
            <PhotoCanvas
              activeTool={tool}
              onDenied={denyTools}
              held={held}
              temporaryMove={temporaryMove}
              wheelZooms={prefs.zoomWithWheel}
              cursor={toolCursor ?? (outline != null ? 'none' : null)}
              viewport={viewport}
              canvas={canvas}
              viewMemory={viewMemory}
              panBounds={panBounds}
              tool={trackedTool}
              decorate={overlays.decorate}
              animated={overlays.animated}
              onView={(next, nextStats) => {
                setZoom(next.zoom)
                setShown((before) => (before && before.zoom === next.zoom && before.width === next.width && before.height === next.height ? before : { zoom: next.zoom, width: next.width, height: next.height }))
                view.set(next)
                // The view, for the scroll test's checks (zoom and the document point at the top left).
                workspace.current?.setAttribute('data-view', `${next.zoom} ${next.x} ${next.y}`)
                if (nextStats) setStats(nextStats)
              }}
            >
              {outline != null ? <BrushCursor pointer={canvasPointer} diameter={outline} hardness={TOOL_FIELDS[tool].includes('hardness') ? options.hardness : 100} /> : null}
              <TypeEditor session={typeSession} view={view} label={t('type.editing')} onAction={(kind) => void typeAction(kind)} />
              {textDraft ? <TextField key={textDraft.id} draft={textDraft} submitting={textPending} focusOnMount={focusText.current} size={(options.fontSize * (state?.resolution ?? 72)) / 72} font={options.font} fontStyle={options.fontStyle} view={view} color={fg} placeholder={t('typePlaceholder')} label={phrase(textDraft.orientation === 'vertical' ? 'Vertical Type Tool' : 'Horizontal Type Tool')} actions={textActions} onTextChange={(text) => setTextDirty(text.trim().length > 0)} onCommit={placeText} onCancel={cancelText} /> : null}
              {/* The scrollbars: there only while the picture and its margin are longer than the view. */}
              <CanvasScrollbars views={view} content={(shownView) => pictureArea(state?.width ?? 0, state?.height ?? 0, shownView.zoom)} canvas={canvas} picture={state ? { width: state.width, height: state.height } : null} />
            </PhotoCanvas>
            <BusyIndicator busy={busy} t={t} onCancel={() => {
              const job = busy.cancel()
              if (job) toast('info', t('busy.cancelled', { label: job.label }))
            }} />
            {floatingPanels.length ? <FloatingPanels kit="kit.photo" store="floating" panels={floatingPanels} onClose={closeFloating} focused={floatingFocus} onFocus={setFloatingFocus} defaultSize={{ width: 300, height: 420 }} minSize={{ width: 260, height: 220 }} testId="photo-floating" /> : null}
          </div>
          {rasterize ? (
            <RasterizePrompt
              message={rasterize.message}
              onCancel={() => {
                setRasterize(null)
                workspace.current?.focus()
              }}
              onConfirm={() => {
                setRasterize(null)
                workspace.current?.focus()
                const current = () => rasterize.generation === toolGenerationNow.current
                void rasterizeThenPaint(rasterize, (command, params) => runTool(command, params, null, current), current)
              }}
            />
          ) : null}
          <ErrorNotices t={t} />
          {help ? <HelpDialog topic={help} version={manifest.version} onClose={() => setHelp(null)} /> : null}
          {brushPicker ? <BrushPickerPopover at={brushPicker} options={brushOptions} onClose={() => setBrushPicker(null)} /> : null}
          <CanvasMenu at={canvasMenu?.at ?? null} rows={canvasMenu?.rows ?? []} phrase={phrase} onChoose={(row) => chooseFromCanvasMenuNow.current(row, canvasMenu?.point ?? null)} onClose={() => setCanvasMenu(null)} />
        </EditorFrame>
      </GradientEditorProvider>
      </ColorPickerProvider>
      </DialogsProvider>
      </TypeEditingContext.Provider>
    </KitEditorProvider>
  )
}
