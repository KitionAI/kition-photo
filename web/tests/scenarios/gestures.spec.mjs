import { test, expect } from '@playwright/test'

// Browser-level tool contracts. The command sink captures a dense stroke before WASM so the
// test isolates input handling without spending CI time rasterizing 200,000 samples.
test('GEST01: a dense stroke reaches the command sink without a stack overflow', async ({ page }) => {
  await page.goto('/dev.html')
  const calls = await page.evaluate(async () => {
    const { brushTool } = await import('/src/tools/canvas/paint.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const calls = []
    const env = {
      options: { ...DEFAULT_OPTIONS, size: 20 }, foreground: '#000000', background: '#ffffff',
      hasLayerEffects: false,
      documentSize: { width: 128, height: 96 },
      run: async (command, params, damage) => {
        calls.push({ command, count: params.points.length, first: params.points[0], last: params.points.at(-1), damage })
        return true
      },
    }
    const context = { overlay: { add: () => {} }, viewport: { view: { zoom: 1 } } }
    let pointer = { x: 16, y: 24, pressure: 0.5, alt: false }
    const gesture = brushTool('Brush', 'paint.stroke', env).begin(pointer, context)
    for (let i = 1; i < 200_000; i++) {
      pointer = { ...pointer, x: i % 2 ? 96 : 16, y: i % 2 ? 72 : 24 }
      gesture.move(pointer, context)
    }
    gesture.end(pointer, context)
    return calls
  })
  expect(calls).toEqual([{
    command: 'paint.stroke', count: 200_000, first: [16, 24, 0.5], last: [96, 72, 0.5],
    damage: { x0: 4, y0: 12, x1: 108, y1: 84 },
  }])
})

for (const tool of ['QuickSelection', 'Lasso', 'DirectSelection']) {
  test(`GEST02: ${tool} includes a newer release position`, async ({ page }) => {
    await page.goto('/dev.html')
    const calls = await page.evaluate(async (tool) => {
      const { quickSelectionTool } = await import('/src/tools/canvas/paint.ts')
      const { lassoTool } = await import('/src/tools/canvas/lasso.ts')
      const { directSelectionTool } = await import('/src/tools/canvas/shape.ts')
      const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
      const calls = []
      const env = {
        options: { ...DEFAULT_OPTIONS },
        run: async (command, params) => { calls.push({ command, params }); return true },
        query: async () => ({ path: { subpaths: [{ closed: false, knots: [
          { anchor: [16, 24], in: [16, 24], out: [16, 24], smooth: false },
          { anchor: [96, 24], in: [96, 24], out: [96, 24], smooth: false },
        ] }] } }),
      }
      const context = { overlay: { draw: () => {} }, viewport: { view: { zoom: 1 } } }
      const start = { x: 16, y: 24, pressure: 0.5, alt: false, shift: false }
      const factory = { QuickSelection: quickSelectionTool, Lasso: lassoTool, DirectSelection: directSelectionTool }[tool]
      const gesture = factory(env).begin(start, context)
      // A lasso needs three vertices; the last one arrives only with release.
      if (tool === 'Lasso') gesture.move({ ...start, x: 96 }, context)
      await gesture.end({ ...start, x: 96, y: 72 }, context)
      return calls
    }, tool)
    expect(calls).toHaveLength(1)
    if (tool === 'DirectSelection') {
      expect(calls[0]).toEqual({ command: 'path.moveAnchors', params: { name: 'work', anchors: [[0, 0]], move: [80, 48] } })
    } else {
      expect(calls[0]).toMatchObject({
        command: tool === 'Lasso' ? 'select.lasso' : 'select.quick',
        params: { points: tool === 'Lasso' ? [[16, 24], [96, 24], [96, 72]] : [[16, 24], [96, 72]] },
      })
    }
  })
}

test('GEST03: releasing a brush click at the same position keeps a single dab', async ({ page }) => {
  await page.goto('/dev.html')
  const points = await page.evaluate(async () => {
    const { brushTool } = await import('/src/tools/canvas/paint.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    let points
    const env = {
      options: { ...DEFAULT_OPTIONS }, foreground: '#000000',
      run: async (_command, params) => { points = params.points; return true },
    }
    const context = { overlay: { add: () => {} }, viewport: { view: { zoom: 1 } } }
    const pointer = { x: 16, y: 24, pressure: 0.5, alt: false }
    const gesture = brushTool('Brush', 'paint.stroke', env).begin(pointer, context)
    gesture.end(pointer, context)
    return points
  })
  expect(points).toEqual([[16, 24, 0.5]])
})

test('GEST04: a Pen handle follows the release position without a final move', async ({ page }) => {
  await page.goto('/dev.html')
  const calls = await page.evaluate(async () => {
    const { penTool } = await import('/src/tools/canvas/shape.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const calls = []
    const env = {
      options: { ...DEFAULT_OPTIONS, penMode: 'path' }, pending: { current: null },
      run: async (command, params) => { calls.push({ command, params }); return true },
    }
    const context = { overlay: { draw: () => {} }, viewport: { view: { zoom: 1 } } }
    const start = { x: 32, y: 32, pressure: 0.5 }
    const tool = penTool(env)
    const first = tool.begin(start, context)
    first.end?.({ ...start, x: 48, y: 40 }, context)
    const next = { ...start, x: 96, y: 64 }
    tool.begin(next, context).end?.(next, context)
    env.pending.current.finish()
    return calls
  })
  expect(calls).toEqual([{
    command: 'path.set', params: { name: 'work', path: { subpaths: [{ closed: false, knots: [
      { anchor: [32, 32], in: [16, 24], out: [48, 40], smooth: true },
      { anchor: [96, 64], in: [96, 64], out: [96, 64], smooth: false },
    ] }] } },
  }])
})

for (const scenario of [
  { name: 'release without move', moves: [], end: 100, expected: 2 },
  { name: 'release after partial move', moves: [25], end: 100, expected: 2 },
  { name: 'release to the left', moves: [-25], end: -100, expected: 0.5 },
  { name: 'return to press', moves: [100], end: 0, expected: 1 },
  { name: 'ordinary click', moves: [], end: 0, expected: 2 },
  { name: 'Alt click', moves: [], end: 0, alt: true, expected: 0.5 },
]) {
  test(`GEST05: scrubby zoom ${scenario.name}`, async ({ page }) => {
    await page.goto('/dev.html')
    const result = await page.evaluate(async ({ moves, end, alt = false }) => {
      const { zoomTool } = await import('/src/tools/canvas/view.ts')
      const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
      const calls = []
      const viewport = {
        view: { zoom: 1 },
        zoomTo: (zoom, x, y) => { viewport.view.zoom = zoom; calls.push([zoom, x, y]) },
      }
      const context = { viewport }
      const start = { x: 64, y: 48, clientX: 200, clientY: 150, alt }
      const gesture = zoomTool({ options: { ...DEFAULT_OPTIONS, scrubbyZoom: true } }).begin(start, context)
      for (const dx of moves) gesture.move({ ...start, clientX: start.clientX + dx }, context)
      gesture.end({ ...start, clientX: start.clientX + end }, context)
      return { zoom: viewport.view.zoom, last: calls.at(-1), count: calls.length }
    }, scenario)
    expect(result.zoom).toBeCloseTo(scenario.expected, 10)
    expect(result.last).toEqual([scenario.expected, 200, 150])
    if (scenario.moves.length === 0) expect(result.count).toBe(1)
  })
}

test('GEST06: fractional marquee coordinates preserve empty and nonempty gestures', async ({ page }) => {
  await page.goto('/dev.html')
  const results = await page.evaluate(async () => {
    const { marqueeTool } = await import('/src/tools/canvas/select.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const results = []
    for (const ellipse of [false, true]) {
      const cases = [
        { name: 'click', dx: 0, dy: 0 },
        { name: 'horizontal', dx: 8, dy: 0 },
        { name: 'vertical', dx: 0, dy: 8 },
        { name: 'small rectangle', dx: 0.25, dy: 0.25 },
        { name: 'small fixed ratio', dx: 0.25, dy: 0.25, style: 'fixedRatio' },
        { name: 'small Shift square', dx: 0.25, dy: 0.25, shift: true },
        { name: 'Shift click', dx: 0, dy: 0, shift: true },
        { name: 'fixed ratio click', dx: 0, dy: 0, style: 'fixedRatio' },
        { name: 'fixed size click', dx: 0, dy: 0, style: 'fixedSize' },
        { name: 'Shift horizontal', dx: 8, dy: 0, shift: true },
      ]
      for (const item of cases) {
        const env = {
          options: { ...DEFAULT_OPTIONS, marqueeStyle: item.style ?? 'normal', marqueeWidth: 40, marqueeHeight: 24 },
          run: async (command, params) => { results.push({ ellipse, name: item.name, command, params }); return true },
        }
        const context = { overlay: {} }
        const start = { x: item.style === 'fixedSize' ? 32 : 32.25, y: item.style === 'fixedSize' ? 24 : 24.25, shift: false, alt: false }
        const gesture = marqueeTool(ellipse ? 'EllipseMarquee' : 'RectMarquee', ellipse, env).begin(start, context)
        gesture.end({ ...start, x: start.x + item.dx, y: start.y + item.dy, shift: item.shift ?? false }, context)
      }
    }
    return results
  })
  expect(results).toHaveLength(20)
  for (const { name, command, params, ellipse } of results) {
    if (['click', 'horizontal', 'vertical', 'fixed ratio click', 'Shift click'].includes(name)) {
      expect({ command, params }, `${ellipse}: ${name}`).toEqual({ command: 'edit.deselectAll', params: {} })
    } else {
      expect(command).toBe('select.rect')
      const [width, height] = name.startsWith('small ') ? [1, 1] : name === 'Shift horizontal' ? [8, 8] : [40, 24]
      expect(params).toMatchObject({ x: 32, y: 24, width, height, ellipse })
    }
  }
})

test('GEST07: fixed-size marquees keep whole-pixel dimensions in every drag direction', async ({ page }) => {
  await page.goto('/dev.html')
  const calls = await page.evaluate(async () => {
    const { marqueeTool } = await import('/src/tools/canvas/select.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const calls = []
    for (const ellipse of [false, true]) {
      for (const [width, height] of [[40, 24], [1, 1]]) {
        for (const [dx, dy] of [[0, 0], [8, 8], [-8, -8], [-8, 8], [8, -8]]) {
          const env = {
            options: { ...DEFAULT_OPTIONS, marqueeStyle: 'fixedSize', marqueeWidth: width, marqueeHeight: height },
            run: async (command, params) => { calls.push({ width, height, dx, dy, ellipse, command, params }); return true },
          }
          const start = { x: 64.25, y: 48.75, shift: false, alt: false }
          const context = { overlay: {} }
          const gesture = marqueeTool(ellipse ? 'EllipseMarquee' : 'RectMarquee', ellipse, env).begin(start, context)
          gesture.end({ ...start, x: start.x + dx, y: start.y + dy }, context)
        }
      }
    }
    return calls
  })
  expect(calls).toHaveLength(20)
  for (const { width, height, dx, dy, ellipse, command, params } of calls) {
    expect(command).toBe('select.rect')
    expect(params).toMatchObject({
      x: dx < 0 ? 64 - width : 64, y: dy < 0 ? 48 - height : 48,
      width, height, ellipse, mode: 'replace',
    })
  }
})


test('GEST08: constrained marquees round extents independently of the fractional anchor', async ({ page }) => {
  await page.goto('/dev.html')
  const calls = await page.evaluate(async () => {
    const { marqueeTool } = await import('/src/tools/canvas/select.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const calls = []
    for (const ellipse of [false, true]) {
      for (const [x, y] of [[64.5, 48], [64.25, 48.75]]) {
        for (const constraint of [
          { style: 'normal', shift: true, width: 1, height: 1, expected: [12, 12] },
          { style: 'fixedRatio', width: 1, height: 1, expected: [12, 12] },
          { style: 'fixedRatio', width: 3, height: 2, expected: [12, 8] },
          { style: 'fixedRatio', width: 2, height: 3, expected: [12, 18] },
          { style: 'fixedRatio', width: 1.5, height: 1, expected: [12, 8] },
        ]) {
          for (const [sx, sy] of [[1, 1], [-1, -1], [-1, 1], [1, -1]]) {
            const env = {
              options: { ...DEFAULT_OPTIONS, marqueeStyle: constraint.style, marqueeWidth: constraint.width, marqueeHeight: constraint.height },
              run: async (command, params) => { calls.push({ ellipse, x, y, sx, sy, constraint, command, params }); return true },
            }
            const context = { overlay: {} }
            const start = { x, y, shift: false, alt: false }
            const gesture = marqueeTool(ellipse ? 'EllipseMarquee' : 'RectMarquee', ellipse, env).begin(start, context)
            gesture.end({ ...start, x: x + sx * 12, y: y + sy * 8, shift: constraint.shift ?? false }, context)
          }
        }
      }
    }
    return calls
  })
  expect(calls).toHaveLength(80)
  for (const { ellipse, x, y, sx, sy, constraint, command, params } of calls) {
    const [width, height] = constraint.expected
    expect(command).toBe('select.rect')
    expect(params, JSON.stringify({ ellipse, x, y, sx, sy, constraint })).toMatchObject({
      x: Math.floor(x) - (sx < 0 ? width : 0), y: Math.floor(y) - (sy < 0 ? height : 0),
      width, height, ellipse, mode: 'replace',
    })
  }
})


test('GEST09: empty selection gestures honor toolbar modes and press modifiers', async ({ page }) => {
  await page.goto('/dev.html')
  const results = await page.evaluate(async () => {
    const { marqueeTool } = await import('/src/tools/canvas/select.ts')
    const { lassoTool } = await import('/src/tools/canvas/lasso.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const results = []
    for (const tool of ['RectMarquee', 'EllipseMarquee', 'Lasso']) {
      for (const mode of ['replace', 'add', 'subtract', 'intersect']) {
        for (const [x, y] of [[32, 24], [32.5, 24.25]]) {
          for (const input of [
            { name: 'toolbar click', shift: false, alt: false, clears: mode === 'replace' || mode === 'intersect' },
            { name: 'Shift click', shift: true, alt: false, clears: false },
            { name: 'Alt click', shift: false, alt: true, clears: false },
            { name: 'Shift+Alt click', shift: true, alt: true, clears: true },
            { name: 'two-point horizontal', dx: 8, shift: false, alt: false, clears: mode === 'replace' || mode === 'intersect' },
          ]) {
            const calls = []
            const env = {
              options: { ...DEFAULT_OPTIONS, selectionMode: mode },
              run: async (command, params) => { calls.push({ command, params }); return true },
            }
            let clears = 0
            const context = { overlay: { clear: () => { clears++ }, draw: () => {} } }
            const start = { x, y, shift: input.shift, alt: input.alt }
            const factory = tool === 'Lasso' ? lassoTool(env) : marqueeTool(tool, tool === 'EllipseMarquee', env)
            const gesture = factory.begin(start, context)
            // Combination uses the press modifiers even when they are released before pointerup.
            const end = { ...start, x: x + (input.dx ?? 0), shift: false, alt: false }
            if (input.dx) gesture.move(end, context)
            gesture.end(end, context)
            results.push({ tool, mode, x, y, input, calls, clears })
          }
        }
      }
    }
    return results
  })
  expect(results).toHaveLength(120)
  for (const { calls, clears, ...scenario } of results) {
    expect(calls, JSON.stringify(scenario)).toEqual(scenario.input.clears ? [{ command: 'edit.deselectAll', params: {} }] : [])
    expect(clears, `No-op must clear the temporary preview: ${JSON.stringify(scenario)}`).toBe(scenario.input.clears ? 0 : 1)
  }
})


test('GEST10: empty rectangle drags clear previews while valid releases still submit', async ({ page }) => {
  await page.goto('/dev.html')
  const results = await page.evaluate(async () => {
    const { shapeTool } = await import('/src/tools/canvas/shape.ts')
    const { cropTool, sliceTool } = await import('/src/tools/canvas/transformLike.ts')
    const { objectSelectionTool } = await import('/src/tools/canvas/select.ts')
    const { DEFAULT_OPTIONS } = await import('/src/tools/options.ts')
    const results = []
    for (const tool of ['Rectangle', 'EllipseShape', 'Triangle', 'Polygon', 'CustomShape', 'Crop', 'Slice', 'ObjectSelection']) {
      for (const input of [
        { name: 'click', end: [24, 24] },
        { name: 'zero width', move: true, end: [24, 64] },
        { name: 'zero height', move: true, end: [88, 24] },
        { name: 'release at start', move: true, end: [24, 24] },
        { name: 'valid release', move: true, end: [96, 72], valid: true },
      ]) {
        const calls = []
        let clears = 0, previews = 0
        const env = {
          options: { ...DEFAULT_OPTIONS }, foreground: '#ff0000', background: '#000000',
          documentSize: { width: 128, height: 96 },
          run: async (command, params) => { calls.push({ command, params }); return true },
          proposeCrop: (params) => calls.push({ command: 'crop proposal', params }),
        }
        const context = { overlay: { clear: () => { clears++ }, draw: () => { previews++ } } }
        const start = { x: 24, y: 24, shift: false, alt: false }
        const factory = tool === 'Crop' ? cropTool(env) : tool === 'Slice' ? sliceTool(env) : tool === 'ObjectSelection' ? objectSelectionTool(env) : shapeTool(tool, env)
        const gesture = factory.begin(start, context)
        if (input.move) gesture.move({ ...start, x: 88, y: 64 }, context)
        gesture.end({ ...start, x: input.end[0], y: input.end[1] }, context)
        results.push({ tool, input, clears, previews, calls })
      }
    }
    return results
  })
  expect(results).toHaveLength(40)
  for (const { tool, input, clears, previews, calls } of results) {
    const label = `${tool}: ${input.name}`
    expect(previews, label).toBe(input.move ? 1 : 0)
    expect(clears, label).toBe(input.valid ? 0 : 1)
    if (!input.valid) expect(calls, label).toEqual([])
    else {
      const command = tool === 'Crop' ? 'crop proposal' : tool === 'Slice' ? 'slice.new' : tool === 'ObjectSelection' ? 'select.object' : tool === 'CustomShape' ? 'shape.presets.place' : 'shape.add'
      expect(calls, label).toHaveLength(1)
      expect(calls[0].command, label).toBe(command)
      expect(calls[0].params, label).toMatchObject(tool === 'Crop' ? { x: 24, y: 24, width: 72, height: 48 } : { rect: [24, 24, 72, 48] })
    }
  }
})
