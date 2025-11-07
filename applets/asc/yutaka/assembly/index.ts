import { Runtime } from '@weilliptic/weil-sdk-asc/assembly/runtime'
import { YutakaContractState } from './yutaka'
import {
  AllowanceArgs,
  ApproveArgs,
  BalanceForArgs,
  TransferArgs,
  TransferFromArgs,
} from '@weilliptic/weil-sdk-asc/assembly/contracts/fungible'
import { WeilError } from '@weilliptic/weil-sdk-asc/assembly/error'
import { Box, WeilValue } from '@weilliptic/weil-sdk-asc/assembly/primitives'
import { Result } from '@weilliptic/weil-sdk-asc/assembly/result'
import { JSONWrapper } from '@weilliptic/weil-sdk-asc/assembly/json/primitives'

export function __free(ptr: usize, len: usize): void {
  Runtime.deallocate(ptr)
}

export function __new(size: usize, id: u32): usize {
  const ptr = Runtime.allocate(size, id)
  return ptr
}

export function init(): void {
  const state = new YutakaContractState()
  const resultValue = state

  const weilValue = WeilValue.newWithStateAndOkValue(state, resultValue)
  const result = Result.Ok<
    WeilValue<YutakaContractState, YutakaContractState>,
    WeilError
  >(weilValue)
  Runtime.setStateAndResult<YutakaContractState, YutakaContractState>(result)
}

export function name(): void {
  const state = Runtime.state<YutakaContractState>()
  Runtime.setOkResult(state.name())
}

export function symbol(): void {
  const state = Runtime.state<YutakaContractState>()
  Runtime.setOkResult(state.symbol())
}

export function decimals(): void {
  const state = Runtime.state<YutakaContractState>()
  Runtime.setOkResult(state.decimals())
}

export function details(): void {
  const state: YutakaContractState = Runtime.state<YutakaContractState>()
  Runtime.setOkResult(state.details().toJSON())
}

export function total_supply(): void {
  const state: YutakaContractState = Runtime.state<YutakaContractState>()
  Runtime.setOkResult(state.totalSupply())
}

export function balance_for(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    YutakaContractState,
    BalanceForArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<YutakaContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<BalanceForArgs>).inner

  if (!args.addr) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'balance_for',
        err_msg: `field "addr" must be provided`,
      }),
    )
    return
  }

  const result = state.balanceFor(args.addr)

  if (result.isOk()) {
    Runtime.setOkResult(result.tryValue().value)
  } else {
    Runtime.setResult(result)
  }
}

export function transfer(): void {
  const stateAndArgs = Runtime.stateAndArgs<YutakaContractState, TransferArgs>()

  const state = (stateAndArgs.elements[0] as JSONWrapper<YutakaContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TransferArgs>).inner

  if (!args.to_addr) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer',
        err_msg: `field "to_addr" must be provided`,
      }),
    )

    return
  }
  if (args.amount === 0) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer',
        err_msg: `field "amount" must be provided`,
      }),
    )

    return
  }
  const res = state.transfer(args.to_addr, args.amount)
  if (res.isOk()) {
    const weilValue = WeilValue.newWithStateAndOkValue(state, "null")
    const result = Result.Ok<
      WeilValue<YutakaContractState, string>,
      WeilError
    >(weilValue)
    Runtime.setStateAndResult<YutakaContractState, string>(result)
  } else {
    const error = res.tryError()
    const result = Result.Err<
      WeilValue<YutakaContractState, Box<i32>>,
      WeilError
    >(error)
    Runtime.setStateAndResult<YutakaContractState, Box<i32>>(result)
  }
}

export function approve(): void {
  const stateAndArgs = Runtime.stateAndArgs<YutakaContractState, ApproveArgs>()

  const state = (stateAndArgs.elements[0] as JSONWrapper<YutakaContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<ApproveArgs>).inner
  if (!args.spender) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'approve',
        err_msg: `field "spender" must be provided`,
      }),
    )

    return
  }
  if (args.value === 0) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'approve',
        err_msg: `field "value" must be provided`,
      }),
    )

    return
  }

  state.approve(args.spender, args.value)
  const weilValue = WeilValue.newWithStateAndOkValue(state, 'ok')
  const result = Result.Ok<WeilValue<YutakaContractState, string>, WeilError>(
    weilValue,
  )
  Runtime.setStateAndResult<YutakaContractState, string>(result)
}

export function transfer_from(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    YutakaContractState,
    TransferFromArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<YutakaContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TransferFromArgs>).inner

  if (!args.from_addr) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "from_addr" must be provided`,
      }),
    )

    return
  }
  if (!args.to_addr) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "to_addr" must be provided`,
      }),
    )

    return
  }
  if (args.value === 0) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "value" must be provided`,
      }),
    )

    return
  }

  const res = state.transferFrom(
    args.from_addr,
    args.to_addr,
    <usize>args.value,
  )

  const weilValue = WeilValue.newWithStateAndOkValue(
    state,
    res.tryValue().value,
  )
  const result = Result.Ok<WeilValue<YutakaContractState, i32>, WeilError>(
    weilValue,
  )
  Runtime.setStateAndResult<YutakaContractState, i32>(result)
}

export function allowance(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    YutakaContractState,
    AllowanceArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<YutakaContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<AllowanceArgs>).inner

  if (!args.owner) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'allowance',
        err_msg: `field "owner" must be provided`,
      }),
    )

    return
  }
  if (!args.spender) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'allowance',
        err_msg: `field "spender" must be provided`,
      }),
    )

    return
  }

  Runtime.setOkResult(state.allowance(args.owner, args.spender))
}

export function method_kind_data(): void {
  const result: Map<string, string> = new Map<string, string>()
  result.set('details', 'query')
  result.set('name', 'query')
  result.set('symbol', 'query')
  result.set('decimals', 'query')
  result.set('total_supply', 'query')
  result.set('balance_for', 'query')
  result.set('transfer', 'mutate')
  result.set('approve', 'mutate')
  result.set('transfer_from', 'mutate')
  result.set('allowance', 'query')
  result.set('__free', 'mutate')
  Runtime.setOkResult(result)
}

export function method_kind(): void {
  method_kind_data()
}
