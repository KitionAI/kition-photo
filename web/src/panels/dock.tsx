import { useMemo, type ReactNode, type RefObject } from 'react'
import { DockArea, useKitEditor, type DockAreaHandle, type DockGroup, type KitCanvasHandle, type KitCanvasView, type KitUiItem } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import type { PhotoColorStore } from '../colors'
import type { ToolOptions } from '../tools/options'
import { DOCK_GROUPS, type DockGroupId, type WindowState } from '../menus/kitMenus'
import { LayersPanel } from './LayersPanel'
import { ChannelsPanel } from './ChannelsPanel'
import { PathsPanel } from './PathsPanel'
import { HistoryPanel } from './HistoryPanel'
import { PropertiesPanel } from './PropertiesPanel'
import { ADJUSTMENT_CONTROLS, AdjustmentsPanel } from './AdjustmentsPanel'
import { ColorPanel } from './ColorPanel'
import { SwatchesPanel } from './SwatchesPanel'
import { NavigatorPanel } from './NavigatorPanel'
import { HistogramPanel } from './HistogramPanel'
import { InfoPanel } from './InfoPanel'
import { CharacterPanel } from './CharacterPanel'
import { ParagraphPanel } from './ParagraphPanel'
import type { DocPoint, Live } from './live'
import type { TypeTarget } from './useTypeStyle'

/**
 * Upstream's right dock (`dock.rs`): six panel groups in Photoshop's Essentials order, each with
 * its tabs, its default height until resized (`Group::default_height`), the compact height it
 * gives way to in a short column (`compact_height`), and its minimum (Layers keeps room for its
 * rows); Layers, the filler at the bottom, asks for about ten rows first (`preferred_fill`). The Window menu shows and hides groups and brings their tabs forward; a tab is a Kit UI panel
 * (`panel:<tab>`) and a group's strip carries its active tab's, so a group shows while any of its
 * tabs does. A group's dock id (`<group>-group`) is the dock's own, never a Kit UI id.
 */
export const DOCK: ReadonlyArray<{ group: DockGroupId; tabs: readonly { id: string; label: string }[]; defaultHeight: number; compactHeight: number; minHeight?: number; preferredFill?: number }> = [
  {
    group: 'color',
    tabs: [
      { id: 'color', label: 'Color' },
      { id: 'swatches', label: 'Swatches' },
      { id: 'gradients', label: 'Gradients' },
      { id: 'patterns', label: 'Patterns' },
    ],
    // Its whole saturation and brightness square shows (120 px under the tab strip) at its default
    // height; the hex, RGB and HSB rows scroll under it (plan 2026-11-24, stage 5).
    defaultHeight: 161,
    compactHeight: 130,
  },
  {
    group: 'properties',
    tabs: [
      { id: 'properties', label: 'Properties' },
      { id: 'adjustments', label: 'Adjustments' },
    ],
    // Include the tab strip so the type layer's Character heading remains fully visible.
    defaultHeight: 260,
    compactHeight: 160,
  },
  {
    group: 'character',
    tabs: [
      { id: 'character', label: 'Character' },
      { id: 'paragraph', label: 'Paragraph' },
    ],
    defaultHeight: 270,
    compactHeight: 160,
  },
  {
    group: 'navigator',
    tabs: [
      { id: 'navigator', label: 'Navigator' },
      { id: 'histogram', label: 'Histogram' },
      { id: 'info', label: 'Info' },
    ],
    defaultHeight: 210,
    compactHeight: 140,
  },
  {
    group: 'history',
    tabs: [
      { id: 'history', label: 'History' },
      { id: 'actions', label: 'Actions' },
      { id: 'layerComps', label: 'Layer Comps' },
    ],
    defaultHeight: 200,
    compactHeight: 130,
  },
  {
    group: 'layers',
    tabs: [
      { id: 'layers', label: 'Layers' },
      { id: 'channels', label: 'Channels' },
      { id: 'paths', label: 'Paths' },
    ],
    defaultHeight: 320,
    compactHeight: 200,
    minHeight: 140,
    // Its header, about four rows and its footer: at 1440 × 900 Color keeps its square, Properties its
    // X, Y, W, H, Opacity and Fill, and Layers the rest (plan 2026-11-24, stage 5). With Color
    // hidden, Properties keeps its default height and the type layer's Character heading shows.
    preferredFill: 347,
  },
]

/** A group's id in the dock (its stored layout, its test ids): never a tab's, so never a Kit UI id. */
export const dockGroupId = (group: DockGroupId) => `${group}-group`

/** Every dock tab as a Kit UI panel (labels are upstream's, through `phrase()`). */
export const DOCK_PANELS: KitUiItem[] = DOCK.flatMap(({ tabs }) => tabs.map((tab) => ({ id: `panel:${tab.id}`, label: tab.label })))

/** The panels' own controls as Kit UI elements, each inside its panel. */
export const PANEL_CONTROLS: KitUiItem[] = [
  ...[
    ['search', 'layers.search'],
    ['filter', 'Kind'],
    ['blend', 'Blend Mode'],
    ['opacity', 'Opacity'],
    ['locks', 'Lock'],
    ['fill', 'Fill'],
    ['list', 'Layers'],
    ['visibility', 'Toggle visibility'],
    ['menu', 'layers.menu'],
    ['link', 'Link layers'],
    ['style', 'Add a layer style'],
    ['mask', 'Add Layer Mask'],
    ['adjustment', 'Create new fill or adjustment layer'],
    ['newGroup', 'Create a new group'],
    ['new', 'Create a new layer'],
    ['delete', 'Delete layer'],
  ].map(([id, label]) => ({ id: `control:layers.${id}`, label, parent: 'panel:layers' })),
  ...[
    ['list', 'Channels'],
    ['visibility', 'Toggle visibility'],
    ['menu', 'channels.menu'],
    ['load', 'Load channel as selection'],
    ['save', 'Save selection as channel'],
    ['new', 'Create new channel'],
    ['delete', 'Delete current channel'],
  ].map(([id, label]) => ({ id: `control:channels.${id}`, label, parent: 'panel:channels' })),
  ...[
    ['list', 'Paths'],
    ['menu', 'paths.menu'],
    ['fill', 'Fill path with foreground color'],
    ['stroke', 'Stroke path with brush'],
    ['load', 'Load path as a selection'],
    ['workPath', 'Make work path from selection'],
    ['vectorMask', 'Add vector mask'],
    ['new', 'Create new path'],
    ['delete', 'Delete path'],
  ].map(([id, label]) => ({ id: `control:paths.${id}`, label, parent: 'panel:paths' })),
  ...[
    ['list', 'History'],
    ['snapshot', 'Create new snapshot'],
    ['delete', 'Delete current state'],
  ].map(([id, label]) => ({ id: `control:history.${id}`, label, parent: 'panel:history' })),
  ...[
    ['canvas', 'Canvas'],
    ['transform', 'Transform'],
    ['opacity', 'Opacity'],
    ['fill', 'Fill'],
    ['align', 'Align and Distribute'],
    ['character', 'Character'],
    ['paragraph', 'Paragraph'],
    ['typeOptions', 'Type Options'],
    ['appearance', 'Appearance'],
    ['shape', 'Shape'],
    ['quickActions', 'Quick Actions'],
    ['adjustment', 'Adjustment Properties'],
    ['masks', 'Masks'],
    ['gradientFill', 'Gradient Fill'],
  ].map(([id, label]) => ({ id: `control:properties.${id}`, label, parent: 'panel:properties' })),
  ...ADJUSTMENT_CONTROLS,
  ...[
    ['chips', 'Colors'],
    ['picker', 'Color'],
  ].map(([id, label]) => ({ id: `control:color.${id}`, label, parent: 'panel:color' })),
  ...[
    ['list', 'Swatches'],
    ['new', 'Create new swatch'],
    ['delete', 'swatches.delete'],
  ].map(([id, label]) => ({ id: `control:swatches.${id}`, label, parent: 'panel:swatches' })),
  ...[
    ['preview', 'Navigator'],
    ['zoom', 'Zoom'],
  ].map(([id, label]) => ({ id: `control:navigator.${id}`, label, parent: 'panel:navigator' })),
  { id: 'control:histogram.channel', label: 'Channel:', parent: 'panel:histogram' },
  ...[
    ['font', 'Font'],
    ['fontStyle', 'options.fontStyle'],
    ['size', 'Font size'],
    ['leading', 'Leading'],
    ['kerning', 'Kerning'],
    ['tracking', 'Tracking'],
    ['verticalScale', 'Vertical scale'],
    ['horizontalScale', 'Horizontal scale'],
    ['baselineShift', 'Baseline shift'],
    ['color', 'Text color'],
    ['styles', 'character.styles'],
  ].map(([id, label]) => ({ id: `control:character.${id}`, label, parent: 'panel:character' })),
  ...[
    ['align', 'options.textAlign'],
    ['startIndent', 'Indent left margin'],
    ['endIndent', 'Indent right margin'],
    ['firstLineIndent', 'Indent first line'],
    ['spaceBefore', 'Add space before paragraph'],
    ['spaceAfter', 'Add space after paragraph'],
    ['hyphenate', 'Hyphenate'],
  ].map(([id, label]) => ({ id: `control:paragraph.${id}`, label, parent: 'panel:paragraph' })),
]

/** The editor state the panels act on. */
export type DockState = {
  info: PhotoInfo
  maskTarget: boolean
  onMaskTarget: (on: boolean) => void
  /** The foreground and background colours (the engine's). */
  colors: PhotoColorStore
  /** The Type tool's options, and their setter (the Properties, Character, and Paragraph panels' type fields set them). */
  typeOptions: ToolOptions
  setTypeOptions: (patch: Partial<ToolOptions>) => void
  /** New text is being typed (the type panels set the tool's options, not a layer). */
  typing: boolean
  /** The pointer over the document (Info), the canvas's view (Navigator), and the canvas to move. */
  pointer: Live<DocPoint>
  view: Live<KitCanvasView | null>
  canvas: RefObject<KitCanvasHandle | null>
}

/** A tab whose panel comes in a later task: it says so, and draws nothing a person could press. */
function Later({ tab }: { tab: string }) {
  const { t } = useKitEditor()
  return (
    <p className="kui-muted photo-empty" data-testid={`photo-panel-later-${tab}`}>
      {t('panels.later')}
    </p>
  )
}

/** The dock beside the canvas: the groups the Window menu shows, their panels drawn from the editor state; `dock` receives its handle (the Window menu brings tabs forward through it). */
export function PhotoDock({ state, windows, dock }: { state: DockState; windows: WindowState; dock: RefObject<DockAreaHandle | null> }) {
  const { phrase } = useKitEditor()
  // The type style's target: the active type layer, else new type (not while new text is typed: the edits are for it).
  const active = state.info.layers.find((layer) => layer.id === state.info.activeLayer)
  const typeTarget: TypeTarget = {
    layer: active?.kind === 'Type' && !state.typing ? active.id : undefined,
    options: state.typeOptions,
    setOptions: state.setTypeOptions,
    foreground: state.colors.foreground,
    setForeground: (foreground) => state.colors.set({ foreground }),
  }
  const content: Record<string, () => ReactNode> = {
    layers: () => <LayersPanel info={state.info} maskTarget={state.maskTarget} onMaskTarget={state.onMaskTarget} />,
    channels: () => <ChannelsPanel info={state.info} maskTarget={state.maskTarget} onMaskTarget={state.onMaskTarget} />,
    paths: () => <PathsPanel info={state.info} foreground={state.colors.foreground} />,
    properties: () => <PropertiesPanel info={state.info} foreground={state.colors.foreground} background={state.colors.background} typeTarget={typeTarget} />,
    // Upstream shows the new layer's settings: the Properties tab comes forward.
    adjustments: () => <AdjustmentsPanel onAdded={() => dock.current?.showTab('properties')} />,
    history: () => <HistoryPanel info={state.info} />,
    color: () => <ColorPanel colors={state.colors} />,
    swatches: () => <SwatchesPanel colors={state.colors} />,
    navigator: () => <NavigatorPanel info={state.info} view={state.view} canvas={state.canvas} />,
    histogram: () => <HistogramPanel info={state.info} />,
    info: () => <InfoPanel info={state.info} pointer={state.pointer} />,
    character: () => <CharacterPanel target={typeTarget} />,
    paragraph: () => <ParagraphPanel target={typeTarget} />,
  }
  const groups: DockGroup[] = DOCK.map(({ group, tabs, defaultHeight, compactHeight, minHeight, preferredFill }) => ({
    id: dockGroupId(group),
    defaultHeight,
    compactHeight,
    minHeight,
    preferredFill,
    tabs: tabs.map((tab) => ({ id: tab.id, label: phrase(tab.label), render: content[tab.id] ?? (() => <Later tab={tab.id} />) })),
  }))
  const shown = useMemo(() => new Set<string>(DOCK_GROUPS.filter((group) => windows.panels[group]).map(dockGroupId)), [windows.panels])
  if (shown.size === 0) return null
  return (
    <aside className="kui-panel photo-dock" data-side="right" data-testid="photo-dock">
      <DockArea kit="kit.photo" groups={groups} shown={shown} handle={dock} testId="photo-dock-area" />
    </aside>
  )
}
