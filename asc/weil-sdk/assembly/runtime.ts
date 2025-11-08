import { JSON } from "json-as";
import { WeilError } from "./error";
import { Result } from "./result";
import { Box, StateArgsValue, WeilValue } from "./primitives";
import { JsonSerializable } from "./json/JsonSerializable";
import { Tuple } from './json/tuples'
import { JSONWrapper } from './json/primitives'


// @ts-ignore
@external("env", "write_collection")
declare function writeCollection(key: ArrayBuffer, val: ArrayBuffer): void;

// @ts-ignore
@external("env", "delete_collection")
declare function deleteCollection(key: ArrayBuffer): i32;

// @ts-ignore
@external("env", "read_collection")
declare function readCollection(key: ArrayBuffer): i32;

// @ts-ignore
@external("env", "read_bulk_collection")
declare function readBulkCollection(prefix: ArrayBuffer): i32;

// @ts-ignore
@external("env", "get_state_and_args")
declare function getStateAndArgs(): i32;

// @ts-ignore
@external("env", "get_state")
declare function getState(): i32;

// @ts-ignore
@external("env", "get_args")
declare function getArgs(): i32;

// @ts-ignore
@external("env", "get_sender")
declare function getSender(): i32;

// @ts-ignore
@external("env", "get_block_height")
declare function getBlockHeight(): i32;

// @ts-ignore
@external("env", "get_block_timestamp")
declare function getBlockTimestamp(): i32;

// @ts-ignore
@external("env", "get_contract_id")
declare function getContractId(): i32;

// @ts-ignore
@external("env", "get_ledger_contract_id")
declare function getLedgerContractId(): i32;

// @ts-ignore
@external("env", "set_state_and_result")
declare function setStateAndResult(ptr: ArrayBuffer): void;

// @ts-ignore
@external("env", "set_state")
declare function setState(state: ArrayBuffer): void;

// @ts-ignore
@external("env", "set_result")
declare function setResult(result: ArrayBuffer): void;

// @ts-ignore
@external("env", "call_contract")
declare function callContract(args: ArrayBuffer): i32;

// @ts-ignore
@external("env", "debug_log")
declare function debugLog(log: ArrayBuffer): void;

@json
class CrossContractCallArgs {
  id: string;
  method_name: string;
  method_args: string;

  constructor(id: string, method_name: string, method_args: string) {
    this.id = id;
    this.method_name = method_name;
    this.method_args = method_args;
  }
}

function readStringFromMemory(ptr: i32): Result<string, WeilError> {
  switch (ptr) {
    case -1:
      return Result.Err<string, WeilError>(WeilError.InvalidWasmModuleError("WASM size limit reached"));
    case -2:
      return Result.Err<string, WeilError>(WeilError.InvalidWasmModuleError("invalid __new function export in module"));
    case -3:
      return Result.Err<string, WeilError>(WeilError.InvalidWasmModuleError("invalid __free function export in module"));
  }

  let isError = load<u8>(ptr);
  let len: i32 = load<u32>(ptr + 1);
  let buffer = new Uint8Array(len);

  for (let i = 0; i < len; ++i) {
    buffer[i] = load<u8>(ptr + 1 + 4 + i);
  }

  let s = String.UTF8.decode(buffer.buffer);

  if (isError) {
    return Result.Err<string, WeilError>(WeilError.fromJSON(s))
  } else {
    return Result.Ok<string, WeilError>(s)
  }
}

function getLengthPrefixedBytesFromResult<T>(result: Result<T, WeilError>): Uint8Array {
  let serializedPayload: string;
  let isError: u8;

  if (result.isErr()) {
    serializedPayload = JSON.stringify(result.tryError());
    isError = 1;
  } else {
    serializedPayload = JSON.stringify(result.tryValue());
    isError = 0;
  }

  return getLengthPrefixedBytesFromString(serializedPayload, isError);
}

function getLengthPrefixedBytesFromString(serializedPayload: string, isError: u8): Uint8Array {
  const payloadBytes = Uint8Array.wrap(String.UTF8.encode(serializedPayload));

  // Allocate a buffer to hold length prefix, error flag, and payload
  const buffer = new Uint8Array(1 + 4 + payloadBytes.length);

  // Set error flag (1 byte)
  buffer[0] = isError;

  // Set length prefix (4 bytes as a little-endian 32-bit integer)
  const length = payloadBytes.length;
  buffer[1] = length & 0xFF;
  buffer[2] = (length >> 8) & 0xFF;
  buffer[3] = (length >> 16) & 0xFF;
  buffer[4] = (length >> 24) & 0xFF;

  // Copy the payload bytes into the buffer
  buffer.set(payloadBytes, 5);

  return buffer;
}

function getLengthPrefixedString<E extends JsonSerializable = WeilError>(s: Result<string, E>): ArrayBuffer {
  let payload: string;
  let isError: u8;

  if (s.isOk()) {
    payload = s.tryValue();
    isError = 0;
  } else {
    payload = s.tryError().toJSON();
    isError = 1;
  }

  let stringBuf = Uint8Array.wrap(String.UTF8.encode(payload))
  let len = stringBuf.byteLength
  let buffer = new ArrayBuffer(1 + 4 + len);
  let dataView = new DataView(buffer);

  dataView.setUint8(0, isError);
  dataView.setUint32(1, len, true);

  for (let i = 0; i < len; ++i) {
    dataView.setInt8(1 + 4 + i, stringBuf[i])
  }

  return buffer
}

export class Memory {
  static writeCollection<V>(key: string, val: V): void {
    let rawKey = getLengthPrefixedString(Result.Ok<string, WeilError>(key));
    let rawVal = getLengthPrefixedString(Result.Ok<string, WeilError>(JSON.stringify<V>(val)));

    writeCollection(rawKey, rawVal)
  }

  static deleteCollection<V>(key: string): V | null {
    let rawKey = getLengthPrefixedString(Result.Ok<string, WeilError>(key));
    let ptr = deleteCollection(rawKey);
    let result = readStringFromMemory(ptr);

    if (result.isOk()) {
      let serializedVal = result.tryValue();
      return JSON.parse<V>(serializedVal)
    } else {
      return null
    }
  }

  static readCollection<V>(key: string): V | null {
    let rawKey = getLengthPrefixedString(Result.Ok<string, WeilError>(key));
    let ptr = readCollection(rawKey);
    let result = readStringFromMemory(ptr);

    if (result.isOk()) {
      let serializedVal = result.tryValue();
      return JSON.parse<V>(serializedVal)
    } else {
      return null
    }
  }
}

export class Runtime {
  static state<T>(): T {
    let ptr = getStateAndArgs();
    let memString = readStringFromMemory(ptr);
    let serializedStateAndArgs = memString.tryValue();
    let stateArgs = JSON.parse<StateArgsValue>(serializedStateAndArgs);

    return JSON.parse<T>(stateArgs.state)
  }

  static args<T>(): T {
    let ptr = getArgs();
    let serializedArgs = readStringFromMemory(ptr).tryValue();

    return JSON.parse<T>(serializedArgs)
  }

  static stateAndArgs<T, U>(): Tuple {
    let ptr = getStateAndArgs();
    let serializedStateAndArgs = readStringFromMemory(ptr).tryValue();

    let stateArgs = JSON.parse<StateArgsValue>(serializedStateAndArgs);

    let state = JSON.parse<T>(stateArgs.state);
    let args = JSON.parse<U>(stateArgs.args);

    return new Tuple([
      new JSONWrapper<T>(state),
      new JSONWrapper<U>(args),
    ])
  }

  static contractId(): string {
    let ptr = getContractId();
    return readStringFromMemory(ptr).tryValue();
  }

  static sender(): string {
    let ptr = getSender();
    return readStringFromMemory(ptr).tryValue();
  }

  static callContract<R>(
    contractId: string,
    methodName: string,
    methodArgs: string | null = null
  ): Result<Box<R>, WeilError> {
    const args = new CrossContractCallArgs(
      contractId,
      methodName,
      methodArgs != null ? methodArgs : "{}"
    );

    const argsBuf: Uint8Array = getLengthPrefixedBytesFromResult(Result.Ok<CrossContractCallArgs, WeilError>(args));
    const resultPtr = callContract(argsBuf.buffer);
    const serializedResult = readStringFromMemory(resultPtr);


    if (serializedResult.isOk()) {
      let serializedValue = serializedResult.tryValue();
      let value: R = JSON.parse<R>(serializedValue)

      return Result.Ok<Box<R>, WeilError>(new Box<R>(value))
    } else {
      const originalError = serializedResult.tryError()

      return Result.Err<Box<R>, WeilError>(
        WeilError.CrossContractCallResultDeserializationError({
          contract_id: contractId,
          method_name: methodName,
          err_msg: originalError.type
        })
      )
    }
  }

  static ledgerContractId(): string {
    let ptr = getLedgerContractId();
    return readStringFromMemory(ptr).tryValue()
  }

  static blockHeight(): u32 {
    let ptr = getBlockHeight();
    return <u32>parseInt(readStringFromMemory(ptr).tryValue());
  }

  static blockTimestamp(): string {
    let ptr = getBlockTimestamp();
    return readStringFromMemory(ptr).tryValue()
  }

  static setState<T>(state: T): void {
    let serializedState = JSON.stringify<T>(state);
    setState(getLengthPrefixedString(Result.Ok<string, WeilError>(serializedState)))
  }

  static setErrorResult<T extends JsonSerializable = WeilError>(error: T): void {
    const weilError = error instanceof WeilError
      ? error as WeilError
      : new WeilError("Error");

    Runtime.setStateAndResult<string, string>(Result.Err<WeilValue<string, string>, WeilError>(weilError));
  }

  static setOkResult<T>(result: T): void {
    // If result is already a string, wrap it in quotes to ensure proper JSON serialization
    if (typeof result === 'string') {
      const weilValue = WeilValue.newWithOkValue(JSON.stringify(result));
      Runtime.setStateAndOkResult(weilValue);
    } else {
      const weilValue = WeilValue.newWithOkValue(result);
      Runtime.setStateAndOkResult(weilValue);
    }
  }

  // this panics for some reason
  static setResult<T, E extends JsonSerializable = WeilError>(result: Result<T, E>): void {
    if (result.isOk()) {
      Runtime.setOkResult(result.tryValue())
    } else {
      Runtime.setErrorResult(result.tryError())
    }
  }

  // static setResult<T>(result: Result<T, WeilError>): void {
  //   if (result.isOk()) {
  //     const value = result.tryValue();
  //     const weilValue = WeilValue.newWithOkValue(value);
  //     Runtime.setStateAndResult(Result.Ok<WeilValue<null, T>, WeilError>(weilValue));
  //   } else {
  //     Runtime.setStateAndResult(Result.Err<WeilValue<null, T>, WeilError>(result.tryError()));
  //   }
  // }

  static setStateAndOkResult<T, U>(weilValue: WeilValue<T, U>): void {
    const rawValue = weilValue.raw();
    const serializedResult = getLengthPrefixedString(
      Result.Ok<string, WeilError>(JSON.stringify(rawValue))
    );
    setStateAndResult(serializedResult);
  }

  static setStateAndErrResult(error: WeilError): void {
    const serializedResult = getLengthPrefixedString(
      Result.Err<string, WeilError>(error)
    );
    setStateAndResult(serializedResult);
  }

  static setStateAndResult<T, U>(result: Result<WeilValue<T, U>, WeilError>): void {
    if (result.isOk()) {
      const weilValue = result.tryValue();
      const rawValue = weilValue.raw();
      const serializedResult = getLengthPrefixedString(
        Result.Ok<string, WeilError>(JSON.stringify(rawValue))
      );
      setStateAndResult(serializedResult);
    } else {
      const error = result.tryError();
      const serializedResult = getLengthPrefixedString(
        Result.Err<string, WeilError>(error)
      );
      setStateAndResult(serializedResult);
    }
  }

  static debugLog(log: string): void {
    debugLog(getLengthPrefixedString(Result.Ok<string, WeilError>(log)))
  }

  static allocate(size: usize, id: u32): usize {
    let ptr = __new(size, id); // Allocate memory block
    __pin(ptr); // Pin the memory to prevent GC
    return ptr; // Return the pointer to the allocated memory
  }

  // Deallocate memory at a specified pointer
  static deallocate(ptr: usize): void {
    __unpin(ptr); // Unpin the memory, allowing it to be garbage collected
  }

}
