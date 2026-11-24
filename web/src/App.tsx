import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import {
  IconButton,
  KitElement,
  rgbToHex,
  EditorFrame,
  KitEditorProvider,
  createKitStrings,
  kitLeases,
  toast,
  useKitEditorState,
  useKitSession,
  type DockAreaHandle,
  type FrameTab,
  type HostConnection,
  type KitViewState,
  type KitCanvasHandle,
  type KitCanvasView,
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
import { photoCanvasTool, type PendingClicks } from './tools/canvas'
import { paintsPixels, rasterizeNeeded, rasterizeThenPaint, withRasterizePrompt, type RasterizeAsk } from './tools/rasterize'
import { RasterizePrompt } from './tools/RasterizePrompt'
import { TOOL_FIELDS, optionsFor, toolParams, type ToolOptions } from './tools/options'
import { PhotoToolRail, type ToolKeys } from './tools/PhotoToolRail'
import { toolFor, type PhotoToolId } from './tools/catalog'
import { usePhotoOverlays, type CropArea } from './overlays'
import { OptionsBar } from './options/OptionsBar'
import { useToolOptions } from './options/store'
import { PhotoDock, dockGroupId } from './panels/dock'
import { createLive, trackPointer, type DocPoint } from './panels/live'
import { usePhotoColors } from './colors'
import { useMaskTarget } from './panels/useMaskTarget'
import { pendingFontChoices, pendingTypeFont } from './panels/useTypeStyle'
import { TextField, type TextDraft, type TextFieldActions } from './TextField'
import { DEFAULT_WINDOWS, windowCommand, zoomStep, type DockTabs } from './menus/kitMenus'
import { hiddenReason } from './menus/hidden'
import { PhotoStatus } from './PhotoStatus'
import { PaintLease, layerScope, useLayerObjectIds, withPaintLease } from './layerLeases'
import { PhotoMenuKeys, photoMenuTabs, type MenuKeys } from './menus/photoMenus'
import { HelpDialog, type HelpTopic } from './menus/HelpDialog'
import { HomeTab } from './HomeTab'
import { usePhotoViewState } from './viewState'
import { PhotoCanvas } from './PhotoCanvas'
import manifest from '../../kit.json'


const ArrowLeftRight = TesseraIcons.ArrowLeftRightIcon
const RotateCcw = TesseraIcons.RotateCcwIcon
const SquareDashed = TesseraIcons.SquareDashedIcon

/** Engine commands a browser editor never shows: the shell runs none of their keys. */
const isHiddenCommand = (command: string) => hiddenReason(command) != null

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
  const { session: photoSession, maskTarget, setMaskTarget } = useMaskTarget(kitSession)
  const commandSequence = useRef(0)
  const pendingCommands = useRef(new Set<Promise<unknown>>())
  const runSession = useCallback((...args: Parameters<typeof photoSession.run>) => {
    commandSequence.current += 1
    const command = photoSession.run(...args)
    pendingCommands.current.add(command)
    const finished = () => { pendingCommands.current.delete(command) }
    void command.then(finished, finished)
    return command
  }, [photoSession.run])
  const session = { ...photoSession, run: runSession }
  const editor = useKitEditorState(session, host, t, 'photo', phrase)
  const { doc, change, state } = session
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
  const [crop, setCrop] = useState<CropArea | null>(null)
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
    setCloneSource(null)
  }, [doc, cancelToolDrafts])
  const { run, query } = editor
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
  // Show Transform Controls: the active layer's box, except the Background's and during a drag.
  const transformBox = tool === 'Move' && options.showTransform && !moving && !activeLayer?.background ? (state?.activeBounds ?? null) : null
  const overlays = usePhotoOverlays(doc, change, state?.hasSelection ?? false, zoom, tool === 'Crop' && crop ? { area: crop, thirds: options.cropOverlay } : null, tool === 'Pen' || tool === 'PathSelection' || tool === 'DirectSelection', transformBox)
  const fg = colors.foreground
  const bg = colors.background
  // The canvas tools read the colours when they act, so a colour field's drag rebuilds no tool.
  const colorsNow = useRef(colors)
  colorsNow.current = colors
  // The tools read the document's state and the mask target when they act, so neither rebuilds a tool.
  const stateNow = useRef(state)
  stateNow.current = state
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
  // [ and ] size the active tool's brush, when it has one.
  const resizeBrush = (scale: (size: number) => number) => {
    if (TOOL_FIELDS[tool].includes('size')) setToolOptions(tool, { size: Math.min(5000, Math.max(1, Math.round(scale(options.size)))) })
  }

  // The session's `run` takes a repaint area, which tells the canvas what to redraw; a failed tool
  // command says why. A tool that paints pixels asks first to rasterize a layer that has none.
  const canvasTool = useMemo(() => {
    const made = photoCanvasTool(tool, {
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
          return active ? { id: active.id, kind: active.kind, bounds: now?.activeBounds ?? null } : null
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
        proposeCrop: setCrop,
        beginText: (pointer, orientation) => {
          // The old draft can be unfocused after visiting its options. Finish it before
          // replacing it; a rejected command keeps the old draft available for retry.
          const draft = { id: ++textDraftId.current, x: pointer.x, y: pointer.y, left: pointer.canvasX, top: pointer.canvasY, orientation }
          const navigation = ++textNavigation.current
          const start = () => {
            if (navigation !== textNavigation.current) return
            focusText.current = true
            setTextDirty(false)
            setTextDraft(draft)
          }
          const actions = textActions.current
          if (actions) void actions.commit(false).then((committed) => { if (committed) start() })
          else start()
        },
        cloneSource,
        setCloneSource,
        pending,
        moving: setMoving,
    })
    const needed = () => (paintsPixels(tool, optionsNow.current.gradientClassic) ? rasterizeNeeded(stateNow.current, maskTargetNow.current) : null)
    // Alt-click with the Clone Stamp or Healing Brush only sets the source: it paints nothing.
    const paints = (pointer: { alt: boolean }) => paintsPixels(tool, optionsNow.current.gradientClassic) && !((tool === 'CloneStamp' || tool === 'Healing') && pointer.alt)
    return withRasterizePrompt(withPaintLease(made, paints, paintLease), tool, needed, (ask) => {
      if (!rasterizeOpen.current) setRasterize({ ...ask, generation: toolGeneration })
      rasterizeOpen.current = true
    })
  }, [cloneSource, paintLease, query, runTool, tool, toolGeneration])
  // The Info panel reads the pointer on every move over the canvas, whatever the tool or the view is doing.
  const trackedTool = useMemo(() => trackPointer(canvasTool, pointer.set), [canvasTool, pointer])

  /** Makes `next` the tool; `PhotoToolRail` has checked that the host allows it. */
  const chooseTool = async (next: PhotoToolId) => {
    const navigation = ++textNavigation.current
    if (next !== tool) {
      // Visiting text options leaves the draft open but no longer focused, so switching tools
      // may not blur it. Finish with the old tool's options and keep focus on the chosen tool.
      const actions = textActions.current
      if (actions && !(await actions.commit(false))) return
      // Several clicks can share the pending submission; only the latest may navigate.
      if (navigation !== textNavigation.current) return
      pending.current?.cancel()
    }
    setTool(next)
    setCrop(null)
  }
  const applyCrop = async () => {
    if (!crop) return
    const area = crop
    // Move focus before disabling Apply, while the command may still be waiting on the worker.
    // Do not focus again on completion: the user may have moved to another control meanwhile.
    workspace.current?.focus()
    setCrop(null)
    if (await run('image.crop', { ...area, ...toolParams('Crop', options) })) {
      canvas.current?.fit()
    }
  }
  const cancelCrop = () => {
    workspace.current?.focus()
    setCrop(null)
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
  const menuTabs = useMemo(() => photoMenuTabs({ windows, dock: dockTabs, onKitCommand: runKitCommand, phrase }), [windows, dockTabs, runKitCommand, phrase])
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
  const placeText = (text: string, focusEditor: boolean) => {
    const draft = textDraft
    if (!draft) return Promise.resolve(false)
    focusAfterText.current = focusEditor
    setTextPending(true)
    const generation = toolGenerationNow.current
    const font = pendingTypeFont(doc)
    const create = (choice: Partial<ToolOptions> = {}) => generation === toolGenerationNow.current
      ? run('type.create', { x: Math.round(draft.x), y: Math.round(draft.y), text, color: fg, orientation: draft.orientation, ...toolParams('Type', { ...options, ...choice }) })
      : Promise.resolve(null)
    // A font selection is an editing intent even before metadata answers. Capture its result
    // for this draft; later option changes still belong to the next draft (UI38).
    return (font ? font.then((choice) => choice ? create(choice) : null) : create()).then((result) => {
      if (result !== null) {
        setTextDirty(false)
        setTextDraft(null)
      }
      // Restore typing only if focus is still in the workspace; preserve a later field choice.
      else focusText.current = document.activeElement === workspace.current
      setTextPending(false)
      return result !== null
    })
  }

  // Every editor Save entry point must finish a draft, including one left open in text options.
  // Repeated requests for the same content share a write; newer edits request a follow-up.
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

  // Photoshop's keys (kit-editor's canvas drops the drag under way on Escape): Enter and Escape finish or drop a crop or a
  // several-click tool (Pen, Polygonal Lasso), [ and ] size the brush, X swaps the colors, D resets
  // them, letters choose tools. An Escape with none of these to drop is not taken, so in Focus it
  // leaves Focus (KitPointer 4.4).
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const command = event.metaKey || event.ctrlKey
    const key = event.key.toLowerCase()
    if (menuKeys.current?.(event)) return true
    if (crop && key === 'enter') void applyCrop()
    else if (crop && key === 'escape') cancelCrop()
    else if (pending.current && key === 'enter') pending.current.finish()
    else if (pending.current && key === 'escape') pending.current.cancel()
    else if (command || event.altKey) return false
    else if (key === '[') resizeBrush((size) => size / 1.2)
    else if (key === ']') resizeBrush((size) => size * 1.2)
    else if (key === 'x') colors.swap()
    else if (key === 'd') colors.reset()
    else return toolKeys.current?.(event) ?? false
    return true
  }

  const railTool = toolFor(tool)

  return (
    <KitEditorProvider editor={{ ...editor, save: saveWithText, session: { ...session, dirty: session.dirty || textDirty } }}>
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
              crop={{ proposed: crop != null, commit: () => void applyCrop(), cancel: cancelCrop }}
              text={{ editing: textDraft != null, submitting: textPending, commit: () => textActions.current?.commit(), cancel: () => textActions.current?.cancel() }}
              cloneSourceSet={cloneSource != null}
              view={{ fit: () => canvas.current?.fit(), actual: () => canvas.current?.zoomTo(1) }}
            />
          ) : undefined
        }
        canvas={canvas}
        statusBar={windows.status}
        hiddenCommands={isHiddenCommand}
        onKeyDown={keyDown}
        status={state ? <PhotoStatus info={state} zoom={zoom} canvas={canvas} tool={tool} /> : null}
        statusEnd={
          <span className="tabular-nums" data-testid="photo-web-stats">
            {[session.openMs != null ? t('stats.open', { ms: session.openMs }) : null, stats ? t('stats.render', { ms: stats.renderMs }) : null].filter(Boolean).join(' · ')}
          </span>
        }
        focusTool={railTool ? { label: phrase(railTool.label), icon: railTool.icon } : null}
        rail={
          <PhotoToolRail active={tool} onChoose={chooseTool} shown={windows.toolbar} keys={toolKeys}>
            <KitElement id="control:colors">
              <div className="photo-colors" title={t('colors')}>
                <label className="photo-color photo-color-background" style={{ background: bg }} title={phrase('Set background color')}>
                  <input type="color" value={bg} onChange={(event) => colors.set({ background: event.target.value })} aria-label={phrase('Set background color')} data-testid="photo-background" />
                </label>
                <label className="photo-color photo-color-foreground" style={{ background: fg }} title={phrase('Set foreground color')}>
                  <input type="color" value={fg} onChange={(event) => colors.set({ foreground: event.target.value })} aria-label={phrase('Set foreground color')} data-testid="photo-foreground" />
                </label>
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
        <PhotoMenuKeys onKitCommand={runKitCommand} keys={menuKeys} />
        <div className="kui-row" ref={workspace} tabIndex={-1} data-testid="photo-workspace">
          <PhotoCanvas
            activeTool={tool}
            onDenied={cancelToolDrafts}
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
            {textDraft ? <TextField key={textDraft.id} draft={textDraft} submitting={textPending} focusOnMount={focusText.current} size={options.fontSize} view={view} color={fg} placeholder={t('typePlaceholder')} label={phrase(textDraft.orientation === 'vertical' ? 'Vertical Type Tool' : 'Horizontal Type Tool')} actions={textActions} onTextChange={(text) => setTextDirty(text.trim().length > 0)} onCommit={placeText} onCancel={cancelText} /> : null}
            {/* The scrollbars: there only while the picture and its margin are longer than the view. */}
            <CanvasScrollbars views={view} content={(shownView) => pictureArea(state?.width ?? 0, state?.height ?? 0, shownView.zoom)} canvas={canvas} />
          </PhotoCanvas>
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
        {help ? <HelpDialog topic={help} version={manifest.version} onClose={() => setHelp(null)} /> : null}
      </EditorFrame>
    </KitEditorProvider>
  )
}
