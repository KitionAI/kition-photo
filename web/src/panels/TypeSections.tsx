import { useEffect, useState } from 'react'
import { type IconComponent } from '@kition/sdk/editor'
import { TesseraIcons } from '@kition/sdk/editor'
import { Checkbox, Choice, IconButton, KitElement, NumberField, useKitEditor } from '@kition/sdk/editor'
import { useFontChoices } from '../options/typeLayer'
import { ALIGNMENTS, KERNING_PRESETS, TOOL_ALIGNMENTS, parseKerning, type Alignment, type TypeEdit, type TypeValues } from '../options/typeStyle'
import { ColorSwatch, Section } from './PropsParts'
import type { TypeStyle } from './useTypeStyle'


const ALargeSmall = TesseraIcons.ALargeSmallIcon
const Bold = TesseraIcons.BoldIcon
const CaseUpper = TesseraIcons.CaseUpperIcon
const Italic = TesseraIcons.ItalicIcon
const Strikethrough = TesseraIcons.StrikethroughIcon
const TextCursor = TesseraIcons.TextCursorIcon
const Underline = TesseraIcons.UnderlineIcon

/**
 * Upstream's type sections (`type_tool::type_sections` and `type_options`), the one place they
 * are drawn: the Character and Paragraph panels show one each, the Properties panel shows both
 * and Type Options for a type layer (`type_properties`). Every field reads and edits through
 * `useTypeStyle`. A section's fields are Kit UI elements (`control:character.<field>`,
 * `control:paragraph.<field>`) wherever they show; `testId` starts each field's test id.
 */
type SectionProps = { style: TypeStyle; testId: string; uiId?: string | null }

/** A labelled number (upstream's `num_field`): Photoshop's glyph before the value, its meaning as the field's name and tooltip, a drag one history step. */
function TypeNumber({ id, glyph, name, value, min, max, unit, step = 0.1, disabled, onChange, testId }: { id: string; glyph: string; name: string; value: number; min: number; max: number; unit?: string; step?: number; disabled?: boolean; onChange: (value: number, final: boolean) => void; testId: string }) {
  return (
    <KitElement id={id}>
      <NumberField label={glyph} name={name} value={value} min={min} max={max} step={step} unit={unit} disabled={disabled} onChange={onChange} testId={testId} />
    </KitElement>
  )
}

/** Upstream's faux styles (Faux Bold, Faux Italic, All Caps, Small Caps, Underline, Strikethrough): a press toggles one. */
const STYLES: readonly (readonly [string, IconComponent, string, (values: TypeValues) => boolean, (values: TypeValues) => TypeEdit])[] = [
  ['fauxBold', Bold, 'Faux Bold', (v) => v.fauxBold, (v) => ({ fauxBold: !v.fauxBold })],
  ['fauxItalic', Italic, 'Faux Italic', (v) => v.fauxItalic, (v) => ({ fauxItalic: !v.fauxItalic })],
  ['allCaps', CaseUpper, 'All Caps', (v) => v.caps === 'all', (v) => ({ caps: v.caps === 'all' ? 'normal' : 'all' })],
  ['smallCaps', ALargeSmall, 'Small Caps', (v) => v.caps === 'small', (v) => ({ caps: v.caps === 'small' ? 'normal' : 'small' })],
  ['underline', Underline, 'Underline', (v) => v.underline, (v) => ({ underline: !v.underline })],
  ['strikethrough', Strikethrough, 'Strikethrough', (v) => v.strikethrough, (v) => ({ strikethrough: !v.strikethrough })],
]

/** Photoshop's editable kerning combo (upstream's `kerning_field`): type a value in 1/1000 em, or Metrics or Optical, and press Enter (or leave); or pick a preset. */
function KerningField({ value, disabled, onChange, testId }: { value: string; disabled: boolean; onChange: (value: string) => void; testId: string }) {
  const { t } = useKitEditor()
  const [text, setText] = useState(value)
  const [editing, setEditing] = useState(false)
  useEffect(() => {
    if (!editing) setText(value)
  }, [editing, value])
  const commit = () => {
    setEditing(false)
    const parsed = parseKerning(text)
    if (parsed !== null && String(parsed) !== value) onChange(String(parsed))
    else setText(value)
  }
  return (
    <KitElement id="control:character.kerning">
      <span className="photo-kerning" title={t('character.kerning')} data-testid={testId}>
        <span className="kui-number-field-label photo-type-glyph" aria-hidden="true">
          <TextCursor size={12} />
        </span>
        <input
          className="kui-field photo-kerning-input"
          type="text"
          value={text}
          disabled={disabled}
          aria-label={t('character.kerning')}
          onChange={(event) => {
            setEditing(true)
            setText(event.target.value)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') commit()
            else if (event.key === 'Escape') {
              setEditing(false)
              setText(value)
            }
          }}
          onBlur={() => editing && commit()}
          data-testid={`${testId}-input`}
        />
        <select className="kui-field photo-kerning-presets" value="" disabled={disabled} aria-label={t('character.kerningPresets')} onChange={(event) => event.target.value && onChange(event.target.value)} data-testid={`${testId}-presets`}>
          <option value="" disabled hidden />
          {KERNING_PRESETS.map((preset) => (
            <option key={preset} value={preset}>
              {preset}
            </option>
          ))}
        </select>
      </span>
    </KitElement>
  )
}

/** Character: font, style, size and leading, kerning and tracking, vertical and horizontal scale, baseline shift and colour, and the faux styles. */
export function CharacterSection({ style, testId, uiId }: SectionProps) {
  const { phrase, t } = useKitEditor()
  const { values, apply, editable, setColor, layer } = style
  const { fonts, styles } = useFontChoices(values.font, values.fontStyle)
  const number = (key: 'size' | 'leading' | 'tracking' | 'verticalScale' | 'horizontalScale' | 'baselineShift') => (value: number, final: boolean) => apply({ [key]: value }, key, final)
  const field = (key: keyof TypeValues) => `${testId}-${key}`
  return (
    <Section id="character" title={phrase('Character')} uiId={uiId} testId={`${testId}-character`}>
      <KitElement id="control:character.font">
        <Choice label={phrase('Font')} quiet value={values.font} choices={fonts} onChange={style.setFont} testId={field('font')} />
      </KitElement>
      <KitElement id="control:character.fontStyle">
        <Choice label={t('options.fontStyle')} quiet value={values.fontStyle} choices={styles} onChange={(fontStyle) => apply({ fontStyle }, 'fontStyle')} testId={field('fontStyle')} />
      </KitElement>
      <div className="photo-type-row">
        {/* The Type tool keeps whole points from 1 (`RANGES.fontSize`); a layer takes tenths from 0.1, as the engine does. */}
        <TypeNumber id="control:character.size" glyph="tT" name={phrase('Font size')} value={values.size} min={layer ? 0.1 : 1} max={1296} unit="pt" onChange={number('size')} testId={field('size')} />
        <TypeNumber id="control:character.leading" glyph="A↕" name={phrase('Leading (set to the font size × auto-leading when Auto)')} value={values.leading} min={0.1} max={5000} unit="pt" disabled={!editable('leading')} onChange={number('leading')} testId={field('leading')} />
      </div>
      <div className="photo-type-row">
        <KerningField value={values.kerning} disabled={!editable('kerning')} onChange={(kerning) => apply({ kerning }, 'kerning')} testId={field('kerning')} />
        <TypeNumber id="control:character.tracking" glyph="VA" name={phrase('Tracking (1/1000 em)')} value={values.tracking} min={-1000} max={10000} step={1} disabled={!editable('tracking')} onChange={number('tracking')} testId={field('tracking')} />
      </div>
      <div className="photo-type-row">
        <TypeNumber id="control:character.verticalScale" glyph="↕T" name={phrase('Vertical scale')} value={values.verticalScale} min={0} max={1000} unit="%" disabled={!editable('verticalScale')} onChange={number('verticalScale')} testId={field('verticalScale')} />
        <TypeNumber id="control:character.horizontalScale" glyph="↔T" name={phrase('Horizontal scale')} value={values.horizontalScale} min={0} max={1000} unit="%" disabled={!editable('horizontalScale')} onChange={number('horizontalScale')} testId={field('horizontalScale')} />
      </div>
      <div className="photo-type-row">
        <TypeNumber id="control:character.baselineShift" glyph="Aª" name={phrase('Baseline shift')} value={values.baselineShift} min={-1296} max={1296} unit="pt" disabled={!editable('baselineShift')} onChange={number('baselineShift')} testId={field('baselineShift')} />
        <KitElement id="control:character.color">
          <span className="photo-type-color">
            <span className="photo-props-label">{phrase('Color')}</span>
            <ColorSwatch label={phrase('Text color')} value={values.color} onChange={setColor} testId={field('color')} />
          </span>
        </KitElement>
      </div>
      <KitElement id="control:character.styles">
        <div className="photo-type-toggles" role="group" aria-label={t('character.styles')}>
          {STYLES.map(([key, icon, label, on, toggle]) => (
            <IconButton key={key} icon={icon} label={phrase(label)} pressed={on(values)} disabled={!editable('fauxBold')} onClick={() => apply(toggle(values), key)} data-testid={`${testId}-${key}`} />
          ))}
        </div>
      </KitElement>
    </Section>
  )
}

/** The Tessera icon of each of Photoshop's seven paragraph alignments. */
const ALIGN_ICONS: Record<Alignment, IconComponent> = {
  left: TesseraIcons.TextAlignStartIcon,
  center: TesseraIcons.TextAlignCenterIcon,
  right: TesseraIcons.TextAlignEndIcon,
  justifyLeft: TesseraIcons.TextJustifyStartIcon,
  justifyCenter: TesseraIcons.TextJustifyCenterIcon,
  justifyRight: TesseraIcons.TextJustifyEndIcon,
  justifyAll: TesseraIcons.TextAlignJustifyIcon,
}

function AlignGlyph({ align }: { align: Alignment }) {
  const Glyph = ALIGN_ICONS[align]
  return <Glyph size={16} aria-hidden="true" />
}

/** Paragraph: the seven alignments, the left, right, and first-line indents, the space before and after, and hyphenation. */
export function ParagraphSection({ style, testId, uiId }: SectionProps) {
  const { phrase, t } = useKitEditor()
  const { values, apply, editable, layer } = style
  const number = (key: 'startIndent' | 'endIndent' | 'firstLineIndent' | 'spaceBefore' | 'spaceAfter') => (value: number, final: boolean) => apply({ [key]: value }, key, final)
  const field = (key: string) => `${testId}-${key}`
  return (
    <Section id="paragraph" title={phrase('Paragraph')} uiId={uiId} testId={`${testId}-paragraph`}>
      <KitElement id="control:paragraph.align">
        <div className="photo-type-toggles" role="group" aria-label={t('options.textAlign')}>
          {ALIGNMENTS.map(([align, label]) => (
            <button
              key={align}
              type="button"
              className="kui-icon-button"
              aria-pressed={values.align === align}
              aria-label={phrase(label)}
              title={phrase(label)}
              // New type keeps the Type tool's three alignments; a justified one is a layer's.
              disabled={!layer && !TOOL_ALIGNMENTS.includes(align)}
              onClick={() => apply({ align }, 'align')}
              data-testid={field(`align-${align}`)}
            >
              <AlignGlyph align={align} />
            </button>
          ))}
        </div>
      </KitElement>
      <div className="photo-type-row">
        <TypeNumber id="control:paragraph.startIndent" glyph="→|" name={phrase('Indent left margin')} value={values.startIndent} min={-1296} max={1296} unit="pt" disabled={!editable('startIndent')} onChange={number('startIndent')} testId={field('startIndent')} />
        <TypeNumber id="control:paragraph.endIndent" glyph="|←" name={phrase('Indent right margin')} value={values.endIndent} min={-1296} max={1296} unit="pt" disabled={!editable('endIndent')} onChange={number('endIndent')} testId={field('endIndent')} />
      </div>
      <div className="photo-type-row">
        <TypeNumber id="control:paragraph.firstLineIndent" glyph="¶→" name={phrase('Indent first line')} value={values.firstLineIndent} min={-1296} max={1296} unit="pt" disabled={!editable('firstLineIndent')} onChange={number('firstLineIndent')} testId={field('firstLineIndent')} />
      </div>
      <div className="photo-type-row">
        <TypeNumber id="control:paragraph.spaceBefore" glyph="↑¶" name={phrase('Add space before paragraph')} value={values.spaceBefore} min={0} max={1296} unit="pt" disabled={!editable('spaceBefore')} onChange={number('spaceBefore')} testId={field('spaceBefore')} />
        <TypeNumber id="control:paragraph.spaceAfter" glyph="¶↓" name={phrase('Add space after paragraph')} value={values.spaceAfter} min={0} max={1296} unit="pt" disabled={!editable('spaceAfter')} onChange={number('spaceAfter')} testId={field('spaceAfter')} />
      </div>
      <KitElement id="control:paragraph.hyphenate">
        <Checkbox label={phrase('Hyphenate')} checked={values.hyphenate} disabled={!editable('hyphenate')} onChange={(hyphenate) => apply({ hyphenate }, 'hyphenate')} testId={field('hyphenate')} />
      </KitElement>
    </Section>
  )
}

/** The anti-aliasing methods (Type › Anti-Alias), upstream's order; Windows LCD and Windows only for a layer that has one. */
export function antiAliasChoices(phrase: (english: string) => string, current: string) {
  return [
    ['none', phrase('None')],
    ['sharp', phrase('Sharp')],
    ['crisp', phrase('Crisp')],
    ['strong', phrase('Strong')],
    ['smooth', phrase('Smooth')],
    ...(current === 'windowsLcd' ? ([['windowsLcd', phrase('Windows LCD')]] as const) : []),
    ...(current === 'windows' ? ([['windows', phrase('Windows')]] as const) : []),
  ] as const
}

/** Type Options (upstream's `type_options`, Properties only): the layer's anti-aliasing and its orientation. */
export function TypeOptionsSection({ style, testId }: { style: TypeStyle; testId: string }) {
  const { phrase } = useKitEditor()
  const vertical = style.layer?.vertical ?? false
  return (
    <Section id="typeOptions" title={phrase('Type Options')} testId={`${testId}-typeOptions`}>
      <div className="photo-props-row">
        <Choice label={phrase('Anti-aliasing method')} quiet value={style.antialias} choices={antiAliasChoices(phrase, style.antialias)} onChange={style.setAntialias} testId={`${testId}-antialias`} />
        <span className="photo-type-toggles">
          {(
            [
              [false, 'Horizontal type'],
              [true, 'Vertical type'],
            ] as const
          ).map(([on, label]) => (
            <IconButton key={label} icon={TextCursor} label={phrase(label)} pressed={vertical === on} className={on ? 'kui-icon-button photo-type-vertical' : 'kui-icon-button'} onClick={() => style.setOrientation(on)} data-testid={`${testId}-orientation-${on ? 'vertical' : 'horizontal'}`} />
          ))}
        </span>
      </div>
    </Section>
  )
}
