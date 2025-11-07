import { Runtime } from '@weilliptic/weil-sdk-asc/assembly/runtime'
import { AsciiArtContractState } from './asciiart'
import { StringError, WeilError } from '@weilliptic/weil-sdk-asc/assembly/error'
import { WeilId } from '@weilliptic/weil-sdk-asc/assembly/collections/weil-id'
import {
  BalanceOfArgs,
  NonFungibleToken,
  Token,
  TokenIdArgs,
  ApproveArgs,
  TransferArgs,
  TransferFromArgs,
  MintArgs,
  IsApprovedForAllArgs,
  SetApproveForAllArgs,
} from '@weilliptic/weil-sdk-asc/assembly/contracts/non-fungible'
import { WeilSet } from '@weilliptic/weil-sdk-asc/assembly/collections/set'
import { WeilValue } from '@weilliptic/weil-sdk-asc/assembly/primitives'
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
  const controllers = new WeilSet<string>(new WeilId(0))
  controllers.add(Runtime.sender())
  const contract = new AsciiArtContractState(
    controllers,
    new NonFungibleToken('AsciiArt'),
  )
  const resultValue = contract
  const weilValue = WeilValue.newWithStateAndOkValue(contract, resultValue)
  const result = Result.Ok<
    WeilValue<AsciiArtContractState, AsciiArtContractState>,
    WeilError
  >(weilValue)
  Runtime.setStateAndResult<AsciiArtContractState, AsciiArtContractState>(
    result,
  )
}

export function name(): void {
  const contract = Runtime.state<AsciiArtContractState>()
  Runtime.setOkResult(contract.inner.name)
}

export function balance_of(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    BalanceOfArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<BalanceOfArgs>).inner

  if (args.addr === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'balance_of',
        err_msg: `field "addr" must be provided`,
      }),
    )

    return
  }

  Runtime.setOkResult(state.inner.balanceOf(args.addr))
}

export function owner_of(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    TokenIdArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TokenIdArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'owner_of',
        err_msg: `field "token_id" must be provided`,
      }),
    )

    return
  }

  const result = state.inner.ownerOf(args.token_id as string)
  if (result.isOk()) {
    Runtime.setOkResult(result.tryValue())
  } else {
    Runtime.setErrorResult(result.tryError())
  }
}

export function details(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    TokenIdArgs
  >()

  Runtime.debugLog('-ASCII- details')

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner

  const args = (stateAndArgs.elements[1] as JSONWrapper<TokenIdArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'details',
        err_msg: `field "token_id" must be provided`,
      }),
    )

    return
  }

  const result = state.inner.details(args.token_id as string)

  if (result.isOk()) {
    Runtime.setOkResult(result.tryValue())
  } else {
    Runtime.setErrorResult(result.tryError())
  }
}

export function approve(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    ApproveArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<ApproveArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'approve',
        err_msg: `field "token_id" must be provided`,
      }),
    )
    return
  }
  if (args.spender === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'approve',
        err_msg: `field "spender" must be provided`,
      }),
    )
    return
  }

  const res = state.inner.approve(
    args.spender as string,
    args.token_id as string,
  )

  if (res.isOk()) {
    const weilValue = WeilValue.newWithStateAndOkValue(state, res.tryValue())
    const result = Result.Ok<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(weilValue)
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  } else {
    const error = res.tryError()
    const result = Result.Err<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(
      WeilError.FunctionReturnedWithError({
        method_name: 'approve',
        err_msg: error.message,
      }),
    )
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  }
}

export function set_approve_for_all(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    SetApproveForAllArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<SetApproveForAllArgs>)
    .inner

  if (args.spender == null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'set_approve_for_all',
        err_msg: `field "spender" must be provided`,
      }),
    )
    return
  }

  state.inner.setApproveForAll(args.spender as string, args.approval as boolean)

  const weilValue = WeilValue.newWithStateAndOkValue(state, 'null')
  const result = Result.Ok<WeilValue<AsciiArtContractState, string>, WeilError>(
    weilValue,
  )
  Runtime.setStateAndResult<AsciiArtContractState, string>(result)
}

export function transfer(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    TransferArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TransferArgs>).inner
  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer',
        err_msg: `field "token_id" must be provided`,
      }),
    )
    return
  }
  if (args.to_addr === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer',
        err_msg: `field "to_addr" must be provided`,
      }),
    )
    return
  }

  const res = state.inner.transfer(
    args.to_addr as string,
    args.token_id as string,
  )
  if (res.isOk()) {
    const weilValue = WeilValue.newWithStateAndOkValue(state, res.tryValue())
    const result = Result.Ok<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(weilValue)
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  } else {
    const error = res.tryError()
    const result = Result.Err<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(
      WeilError.FunctionReturnedWithError({
        method_name: 'transfer',
        err_msg: error.message,
      }),
    )
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  }
}

export function transfer_from(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    TransferFromArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TransferFromArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "token_id" must be provided`,
      }),
    )
    return
  }
  if (args.from_addr === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "from_addr" must be provided`,
      }),
    )
    return
  }
  if (args.to_addr === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'transfer_from',
        err_msg: `field "to_addr" must be provided`,
      }),
    )
    return
  }

  const res = state.inner.transferFrom(
    args.from_addr as string,
    args.to_addr as string,
    args.token_id as string,
  )

  if (res.isOk()) {
    const weilValue = WeilValue.newWithStateAndOkValue(state, res.tryValue())
    const result = Result.Ok<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(weilValue)
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  } else {
    const error = res.tryError()
    const result = Result.Err<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(
      WeilError.FunctionReturnedWithError({
        method_name: 'transfer_from',
        err_msg: error.message,
      }),
    )
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  }
}

export function get_approved(): void {
  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    TokenIdArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<TokenIdArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'get_approved',
        err_msg: `field "token_id" must be provided`,
      }),
    )
    return
  }

  Runtime.setOkResult(state.inner.getApproved(args.token_id as string))
}

export function is_approved_for_all(): void {
  // const nonNullableArgs = Runtime.args<NonNullable<IsApprovedForAllArgs>>()
  // why did we need non-nullable args also??

  const stateAndArgs = Runtime.stateAndArgs<
    AsciiArtContractState,
    IsApprovedForAllArgs
  >()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<IsApprovedForAllArgs>)
    .inner

  if (args.spender === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'is_approved_for_all',
        err_msg: `field "spender" must be provided`,
      }),
    )
    return
  }
  if (args.owner === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'is_approved_for_all',
        err_msg: `field "owner" must be provided`,
      }),
    )
    return
  }

  Runtime.setOkResult(
    state.inner.isApprovedForAll(args.owner as string, args.spender as string),
  )
}

export function mint(): void {
  const stateAndArgs = Runtime.stateAndArgs<AsciiArtContractState, MintArgs>()

  const state = (stateAndArgs.elements[0] as JSONWrapper<AsciiArtContractState>)
    .inner
  const args = (stateAndArgs.elements[1] as JSONWrapper<MintArgs>).inner

  if (args.token_id === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'mint',
        err_msg: `field "token_id" must be provided`,
      }),
    )
    return
  }
  if (args.name === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'mint',
        err_msg: `field "name" must be provided`,
      }),
    )
    return
  }
  if (args.title === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'mint',
        err_msg: `field "title" must be provided`,
      }),
    )
    return
  }
  if (args.description === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'mint',
        err_msg: `field "description" must be provided`,
      }),
    )
    return
  }
  if (args.payload === null) {
    Runtime.setErrorResult(
      WeilError.MethodArgumentDeserializationError({
        method_name: 'mint',
        err_msg: `field "payload" must be provided`,
      }),
    )
    return
  }

  if (!state.isController(Runtime.sender())) {
    Runtime.setErrorResult(new StringError('Only controllers can mint'))

    return
  }

  const res = state.inner.mint(
    args.token_id as string,
    new Token(
      args.title as string,
      args.name as string,
      args.description as string,
      args.payload as string,
    ),
  )

  if (res.isErr()) {
    const error = res.tryError()
    const result = Result.Err<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(
      WeilError.FunctionReturnedWithError({
        method_name: 'mint',
        err_msg: error.message,
      }),
    )
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  } else {
    const weilValue = WeilValue.newWithStateAndOkValue(state, res.tryValue())
    const result = Result.Ok<
      WeilValue<AsciiArtContractState, string>,
      WeilError
    >(weilValue)
    Runtime.setStateAndResult<AsciiArtContractState, string>(result)
  }
}

export function yo(a: string): void {}

export function method_kind_data(): void {
  const result: Map<string, string> = new Map<string, string>()
  result.set('name', 'query')
  result.set('balance_of', 'query')
  result.set('owner_of', 'query')
  result.set('details', 'query')
  result.set('approve', 'mutate')
  result.set('set_approve_for_all', 'mutate')
  result.set('transfer', 'mutate')
  result.set('transfer_from', 'mutate')
  result.set('get_approved', 'query')
  result.set('is_approved_for_all', 'query')
  result.set('mint', 'mutate')
  result.set('__free', 'mutate')
  Runtime.setOkResult(result)
}

export function method_kind(): void {
  method_kind_data()
}
