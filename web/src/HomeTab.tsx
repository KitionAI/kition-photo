import { TesseraIcons } from '@kition/sdk/editor'
import { CommandButton, CommandItem, MenuSeparator, RibbonButton, RibbonGroup, RibbonMenu, RibbonStack, useKitEditor, useUi } from '@kition/sdk/editor'
import { PHOTO_TOOLS, type PhotoToolId } from './tools/catalog'
import { useDialogs } from './dialogs/DialogHost'


const Brush = TesseraIcons.BrushIcon
const Crop = TesseraIcons.CropIcon
const FlipHorizontal2 = TesseraIcons.TrianglesCenterlineDashedHorizontalIcon
const Layers = TesseraIcons.LayersIcon
const PaintBucket = TesseraIcons.PaintBucketIcon
const RotateCw = TesseraIcons.RotateCwIcon
const Scaling = TesseraIcons.ScalingIcon
const SlidersHorizontal = TesseraIcons.SlidersHorizontalIcon
const Sparkles = TesseraIcons.SparklesIcon
const SquareDashedMousePointer = TesseraIcons.SquareDashedMousePointerIcon
const SquarePlus = TesseraIcons.SquarePlusIcon

/** Image ▸ Adjustments as Photoshop's Adjustments panel makes them: adjustment layers, edited in Properties. */
const ADJUSTMENT_LAYERS = ['brightnessContrast', 'levels', 'curves', 'exposure', 'vibrance', 'hueSaturation', 'colorBalance', 'blackWhite', 'photoFilter', 'gradientMap']
const FILTERS = ['filter.blur.gaussianBlur', 'filter.sharpen.unsharpMask', 'filter.sharpen.smartSharpen', 'filter.noise.addNoise', 'filter.noise.reduceNoise', 'filter.stylize.oilPaint']
const TRANSFORMS = ['edit.transform.again', 'edit.transform.rotate180', 'edit.transform.rotate90Cw', 'edit.transform.rotate90Ccw', 'edit.transform.flipHorizontal', 'edit.transform.flipVertical', 'edit.transform.warp']

/**
 * One of Photo's tools, chosen as the rail chooses it (the host may hide, disable, or deny it).
 * The rail always shows the tools, so they are not `primary`: Classic's ribbon has them, Single
 * line's row does not repeat them.
 */
function HomeTool({ id, tool, onTool, primary, size = 'large' }: { id: PhotoToolId; tool: PhotoToolId; onTool: (id: PhotoToolId) => void; primary?: boolean; size?: 'large' | 'small' }) {
  const { phrase } = useKitEditor()
  const ui = useUi()
  const entry = PHOTO_TOOLS.find((item) => item.id === id)
  const state = ui.state(`tool:${id}`)
  if (!entry || state.hidden) return null
  return (
    <RibbonButton
      icon={entry.icon}
      label={phrase(entry.label)}
      size={size}
      pressed={tool === id}
      disabled={state.disabled}
      onClick={() => {
        if (!state.denied) onTool(id)
      }}
      data-testid={`photo-home-tool-${id}`}
      data-kit-ui-id={`tool:${id}`}
      primary={primary}
    />
  )
}

/**
 * Photo's Home tab: the tasks done most, in every display mode (a drop-down panel in Menus):
 * layers, crop and transform, adjustments (as adjustment layers), filters, selections, and paint.
 * Menu names reuse upstream's words, which the phrase tables translate. Commands run as their menu
 * rows do: one that asks first opens its dialog.
 */
export function HomeTab({ tool, onTool }: { tool: PhotoToolId; onTool: (id: PhotoToolId) => void }) {
  const { phrase: p, t } = useKitEditor()
  // A "…" command opens its dialog, as its menu row does.
  const { invoke } = useDialogs()
  return (
    <>
      <RibbonGroup label={p('Layer')} icon={Layers}>
        {/* Upstream's row reads "Layer…" under Layer ▸ New; on its own it needs the verb. */}
        <CommandButton onRun={invoke} command="layer.new.layer" label={t('home.newLayer')} icon={SquarePlus} size="large" primary />
        <RibbonStack>
          <CommandButton onRun={invoke} command="layer.new.layerViaCopy" icon={Layers} />
          <CommandButton onRun={invoke} command="layer.duplicate" icon={Layers} />
          <CommandButton onRun={invoke} command="layer.mergeDown" icon={Layers} />
        </RibbonStack>
      </RibbonGroup>

      <RibbonGroup label={p('Image')} icon={Crop}>
        <HomeTool id="Crop" tool={tool} onTool={onTool} />
        <RibbonMenu icon={RotateCw} label={p('Transform')} size="large" testId="photo-home-transform" primary>
          {TRANSFORMS.map((command) => (
            <CommandItem key={command} onRun={invoke} command={command} />
          ))}
        </RibbonMenu>
        <RibbonStack>
          <CommandButton onRun={invoke} command="image.imageSize" icon={Scaling} primary />
          <CommandButton onRun={invoke} command="image.canvasSize" icon={Crop} />
          <CommandButton onRun={invoke} command="edit.transform.flipHorizontal" icon={FlipHorizontal2} />
        </RibbonStack>
      </RibbonGroup>

      <RibbonGroup label={p('Adjustments')} icon={SlidersHorizontal}>
        <RibbonMenu icon={SlidersHorizontal} label={p('Adjustments')} size="large" testId="photo-home-adjustments" primary>
          {/* As Photoshop's Adjustments panel: the layer comes at once, its settings in Properties. */}
          {ADJUSTMENT_LAYERS.map((kind) => (
            <CommandItem key={kind} command={`layer.newAdjustmentLayer.${kind}`} />
          ))}
          <MenuSeparator />
          <CommandItem onRun={invoke} command="image.autoTone" />
          <CommandItem onRun={invoke} command="image.autoContrast" />
          <CommandItem onRun={invoke} command="image.autoColor" />
          <MenuSeparator />
          <CommandItem onRun={invoke} command="image.adjustments.invert" />
          <CommandItem onRun={invoke} command="image.adjustments.desaturate" />
        </RibbonMenu>
        <RibbonMenu icon={Sparkles} label={p('Filter')} size="large" testId="photo-home-filters" primary>
          <CommandItem onRun={invoke} command="filter.lastFilter" />
          <MenuSeparator />
          {FILTERS.map((command) => (
            <CommandItem key={command} onRun={invoke} command={command} />
          ))}
        </RibbonMenu>
      </RibbonGroup>

      <RibbonGroup label={p('Select')} icon={SquareDashedMousePointer}>
        <HomeTool id="RectMarquee" tool={tool} onTool={onTool} />
        <RibbonStack>
          <HomeTool id="Lasso" tool={tool} onTool={onTool} size="small" />
          <HomeTool id="QuickSelection" tool={tool} onTool={onTool} size="small" />
        </RibbonStack>
        <RibbonMenu icon={SquareDashedMousePointer} label={p('Select')} size="large" testId="photo-home-select" primary>
          <CommandItem onRun={invoke} command="edit.selectAll" />
          <CommandItem onRun={invoke} command="edit.deselectAll" />
          <CommandItem onRun={invoke} command="select.inverse" />
          <MenuSeparator />
          <CommandItem onRun={invoke} command="select.subject" />
          <CommandItem onRun={invoke} command="select.sky" />
          <CommandItem onRun={invoke} command="select.colorRange" />
          <MenuSeparator />
          <CommandItem onRun={invoke} command="select.modify.feather" />
          <CommandItem onRun={invoke} command="select.transformSelection" />
        </RibbonMenu>
      </RibbonGroup>

      <RibbonGroup label={t('home.paint')} icon={Brush}>
        <HomeTool id="Brush" tool={tool} onTool={onTool} />
        <HomeTool id="Eraser" tool={tool} onTool={onTool} />
        <CommandButton onRun={invoke} command="edit.fill" icon={PaintBucket} size="large" primary />
        <HomeTool id="Type" tool={tool} onTool={onTool} />
        <RibbonStack>
          <HomeTool id="Gradient" tool={tool} onTool={onTool} size="small" />
          <CommandButton onRun={invoke} command="edit.stroke" icon={PaintBucket} />
        </RibbonStack>
      </RibbonGroup>
    </>
  )
}
