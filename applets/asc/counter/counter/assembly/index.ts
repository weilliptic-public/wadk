import { Runtime } from '@weilliptic/weil-sdk-asc/assembly/runtime'
import { WeilValue } from '@weilliptic/weil-sdk-asc/assembly/primitives'
import { CounterContractState, SetValueArgs } from './counter'
import { JSONWrapper } from '@weilliptic/weil-sdk-asc/assembly/json/primitives'

export function __free(ptr: usize, len: usize): void {
  Runtime.deallocate(ptr)
}

export function __new(size: usize, id: u32): usize {
  const ptr = Runtime.allocate(size, id)
  return ptr
}

export function init(): void {
  const state = new CounterContractState()
  Runtime.setStateAndOkResult(new WeilValue(state, true))
}

export function get_count(): void {
  const state = Runtime.state<CounterContractState>()
  const count = state.getCount()

  Runtime.setOkResult(count)
}

export function increment(): void {
  const state = Runtime.state<CounterContractState>()
  state.increment()

  Runtime.setStateAndOkResult(new WeilValue(state, true))
}

export function set_value(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    CounterContractState,
    SetValueArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<CounterContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<SetValueArgs>).inner
  state.setValue(args.val)

  Runtime.setStateAndOkResult(new WeilValue(state, true))
}

export function method_kind_data(): void {
  const result: Map<string, string> = new Map<string, string>()
  result.set('get_count', 'query')
  result.set('increment', 'mutate')
  result.set('set_value', 'mutate')
  result.set('__free', 'mutate')
  Runtime.setOkResult(result)
}

export function method_kind(): void {
  method_kind_data()
}
