import { Runtime } from '@weilliptic/weil-sdk-asc/assembly/runtime'
import { CrossCounterContractState, CrossContractArgs } from './counter'
import { WeilValue } from '@weilliptic/weil-sdk-asc/assembly/primitives'
import { JSONWrapper } from '@weilliptic/weil-sdk-asc/assembly/json/primitives'

export function __free(ptr: usize, len: usize): void {
  Runtime.deallocate(ptr)
}

export function __new(size: usize, id: u32): usize {
  const ptr = Runtime.allocate(size, id)
  return ptr
}

export function init(): void {
  const state = new CrossCounterContractState()
  Runtime.setStateAndOkResult(new WeilValue(state, true))
}

export function fetch_counter_from(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    CrossCounterContractState,
    CrossContractArgs
  >()

  const state = (
    stateAndArgs.elements[0] as JSONWrapper<CrossCounterContractState>
  ).inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<CrossContractArgs>)
    .inner

  const result = state.fetch_counter_from(args.contract_id)

  Runtime.setResult(result)
}

export function increment_counter_of(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    CrossCounterContractState,
    CrossContractArgs
  >()

  const state = (
    stateAndArgs.elements[0] as JSONWrapper<CrossCounterContractState>
  ).inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<CrossContractArgs>)
    .inner

  const result = state.increment_counter_of(args.contract_id)

  if (result.isOk()) {
    Runtime.setStateAndOkResult(new WeilValue(state, result.tryValue()))
  } else {
    Runtime.setStateAndErrResult(result.tryError())
  }
}

export function method_kind_data(): void {
  const result: Map<string, string> = new Map<string, string>()
  result.set('fetch_counter_from', 'query')
  result.set('increment_counter_of', 'mutate')
  result.set('__free', 'mutate')
  Runtime.setOkResult(result)
}

export function method_kind(): void {
  method_kind_data()
}
