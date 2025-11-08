// The entry file of your WebAssembly module.

import { Runtime } from './runtime'
import { WeilVec } from './collections/vec'
import { WeilMap } from './collections/map'
import { WeilSet } from './collections/set'
import { JSON } from 'json-as'
import { WeilId } from './collections/weil-id'

const vec = new WeilVec<string>(new WeilId(10))
const map = new WeilMap<string, string>(new WeilId(11))
const set = new WeilSet<string>(new WeilId(12))

export function init(): void {
  Runtime.debugLog('Hello, reached here!')
  Runtime.setState('{"counter: 0"}')

  vec.push('hello')
  vec.push('world')

  map.set('hello', 'world')

  set.add('hello')
  set.add('world')
}

export function test(): void {
  Runtime.debugLog('--debugme--')

  map.delete('hello')

  set.delete('hello')
  set.delete('world')

  vec.pop()
  vec.pop()

  // Runtime.setResult<string>('--debugme--')
}

export function toss_collections_around(): void {
  const jsonState = new Map<string, string>()

  jsonState.set('vec0', <string>(vec.get(0) || '-'))
  jsonState.set('vec1', <string>(vec.get(1) || '-'))

  jsonState.set('mapH', <string>(map.get('hello') || '-'))
  jsonState.set('setH', set.has('hello') ? 'hello' : '-')
  jsonState.set('setW', set.has('world') ? 'world' : '-')

  Runtime.setState(JSON.stringify(jsonState))
  const state = Runtime.state<string>()

  Runtime.debugLog(state)

  // const result = JSON.stringify(jsonState)
  // Runtime.setResult<string>(result)
}

export function get_counter(): void {
  let state = Runtime.state<string>()

  Runtime.debugLog(state)
}

export function yo(s: string): void {}
