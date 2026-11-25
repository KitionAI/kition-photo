import { readFileSync, mkdirSync, writeFileSync } from 'node:fs'

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'))
const scenarios = read('./cases.json')
const inventory = read('../../parity/photo-status.json')
// The inventory names commands as upstream does; the scenarios run the engine's ids.
const engineIds = read('../../parity/photo-upstream-ids.json')
const engineId = (command) => engineIds[command] ?? command
const commands = new Map()
for (const scenario of scenarios) {
  for (const { command, error } of scenario.steps) {
    if (!command || error) continue
    const ids = commands.get(command) ?? new Set()
    ids.add(scenario.id)
    commands.set(command, ids)
  }
}
// This maps command execution, not full UI/menu/option coverage. Never equate the two.
const entries = Object.entries(inventory).map(([key, value]) => ({
  key,
  declaredStatus: value.status,
  engineScenarios: key.startsWith('menu:') ? [...(commands.get(engineId(key.split('/').at(-1))) ?? [])] : [],
}))
const report = {
  note: 'engineScenarios proves that a command participates in a result-checked scenario. It does not prove every parameter, menu interaction, tool or dialog. Browser scenario mapping is in docs/testing/scenarios.md.',
  inventoryEntries: entries.length,
  declared: Object.fromEntries(['done', 'todo', 'hidden'].map((status) => [status, entries.filter((item) => item.declaredStatus === status).length])),
  dataScenarios: scenarios.length,
  successfulCommandIds: commands.size,
  menuEntriesWithScenarioCommands: entries.filter((item) => item.engineScenarios.length).length,
  commands: Object.fromEntries([...commands].sort(([a], [b]) => a.localeCompare(b)).map(([key, ids]) => [key, [...ids]])),
  entries,
}
const out = new URL('../../test-results/', import.meta.url)
mkdirSync(out, { recursive: true })
writeFileSync(new URL('coverage.json', out), JSON.stringify(report, null, 2))
console.log(`${report.dataScenarios} data scenarios; ${report.successfulCommandIds} successful command IDs; ${report.menuEntriesWithScenarioCommands}/${report.inventoryEntries} inventory entries map to those menu commands. Full UI coverage is not implied.`)
