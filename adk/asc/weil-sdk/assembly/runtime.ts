import { JSON } from "json-as";
import { WeilError } from "./error";
import { Result } from "./result";
import { Box, StateArgsValue, WeilValue } from "./primitives";
import { JsonSerializable } from "./json/JsonSerializable";
import { Tuple } from './json/tuples'
import { JSONWrapper } from './json/primitives'


/**
 * External function: Writes a key-value pair to the persistent collection storage.
 * 
 * @param key - The key as an ArrayBuffer
 * @param val - The value as an ArrayBuffer
 */
// @ts-ignore
@external("env", "write_collection")
declare function writeCollection(key: ArrayBuffer, val: ArrayBuffer): void;

/**
 * External function: Deletes a value from the persistent collection storage.
 * 
 * @param key - The key to delete as an ArrayBuffer
 * @returns A pointer to the deleted value in memory, or an error code
 */
// @ts-ignore
@external("env", "delete_collection")
declare function deleteCollection(key: ArrayBuffer): i32;

/**
 * External function: Reads a value from the persistent collection storage.
 * 
 * @param key - The key to read as an ArrayBuffer
 * @returns A pointer to the value in memory, or an error code
 */
// @ts-ignore
@external("env", "read_collection")
declare function readCollection(key: ArrayBuffer): i32;

/**
 * External function: Reads multiple values from the persistent collection storage with a given prefix.
 * 
 * @param prefix - The key prefix to search for as an ArrayBuffer
 * @returns A pointer to the results in memory, or an error code
 */
// @ts-ignore
@external("env", "read_bulk_collection")
declare function readBulkCollection(prefix: ArrayBuffer): i32;

/**
 * External function: Gets both the contract state and method arguments.
 * 
 * @returns A pointer to the state and arguments in memory
 */
// @ts-ignore
@external("env", "get_state_and_args")
declare function getStateAndArgs(): i32;

/**
 * External function: Gets the contract state.
 * 
 * @returns A pointer to the state in memory
 */
// @ts-ignore
@external("env", "get_state")
declare function getState(): i32;

/**
 * External function: Gets the method arguments.
 * 
 * @returns A pointer to the arguments in memory
 */
// @ts-ignore
@external("env", "get_args")
declare function getArgs(): i32;

/**
 * External function: Gets the address of the transaction sender.
 * 
 * @returns A pointer to the sender address in memory
 */
// @ts-ignore
@external("env", "get_sender")
declare function getSender(): i32;

/**
 * External function: Gets the current block height.
 * 
 * @returns A pointer to the block height in memory
 */
// @ts-ignore
@external("env", "get_block_height")
declare function getBlockHeight(): i32;

/**
 * External function: Gets the current block timestamp.
 * 
 * @returns A pointer to the block timestamp in memory
 */
// @ts-ignore
@external("env", "get_block_timestamp")
declare function getBlockTimestamp(): i32;

/**
 * External function: Gets the current contract ID.
 * 
 * @returns A pointer to the contract ID in memory
 */
// @ts-ignore
@external("env", "get_contract_id")
declare function getContractId(): i32;

/**
 * External function: Gets the ledger contract ID.
 * 
 * @returns A pointer to the ledger contract ID in memory
 */
// @ts-ignore
@external("env", "get_ledger_contract_id")
declare function getLedgerContractId(): i32;

/**
 * External function: Sets both the contract state and result.
 * 
 * @param ptr - A pointer to the serialized state and result as an ArrayBuffer
 */
// @ts-ignore
@external("env", "set_state_and_result")
declare function setStateAndResult(ptr: ArrayBuffer): void;

/**
 * External function: Sets the contract state.
 * 
 * @param state - The serialized state as an ArrayBuffer
 */
// @ts-ignore
@external("env", "set_state")
declare function setState(state: ArrayBuffer): void;

/**
 * External function: Sets the method result.
 * 
 * @param result - The serialized result as an ArrayBuffer
 */
// @ts-ignore
@external("env", "set_result")
declare function setResult(result: ArrayBuffer): void;

/**
 * External function: Calls another contract method.
 * 
 * @param args - The serialized call arguments as an ArrayBuffer
 * @returns A pointer to the result in memory, or an error code
 */
// @ts-ignore
@external("env", "call_contract")
declare function callContract(args: ArrayBuffer): i32;

/**
 * External function: Logs a debug message.
 * 
 * @param log - The log message as an ArrayBuffer
 */
// @ts-ignore
@external("env", "debug_log")
declare function debugLog(log: ArrayBuffer): void;

/**
 * Arguments for cross-contract calls.
 */
@json
class CrossContractCallArgs {
  /** The contract ID to call */
  id: string;
  /** The method name to call */
  method_name: string;
  /** The serialized method arguments */
  method_args: string;

  /**
   * Creates a new CrossContractCallArgs instance.
   * 
   * @param id - The contract ID to call
   * @param method_name - The method name to call
   * @param method_args - The serialized method arguments
   */
  constructor(id: string, method_name: string, method_args: string) {
    this.id = id;
    this.method_name = method_name;
    this.method_args = method_args;
  }
}

/**
 * Reads a string from memory at the given pointer.
 * Handles error codes and length-prefixed strings.
 * 
 * @param ptr - The memory pointer to read from
 * @returns A Result containing the string or an error
 */
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

/**
 * Converts a Result to length-prefixed bytes for host communication.
 * 
 * @template T - The type of the success value
 * @param result - The Result to convert
 * @returns A Uint8Array with length prefix and error flag
 */
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

/**
 * Converts a string to length-prefixed bytes with an error flag.
 * 
 * @param serializedPayload - The string to convert
 * @param isError - Whether this represents an error (1) or success (0)
 * @returns A Uint8Array with error flag, length prefix, and payload
 */
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

/**
 * Converts a Result<string, E> to a length-prefixed ArrayBuffer for host communication.
 * 
 * @template E - The error type (must extend JsonSerializable)
 * @param s - The Result to convert
 * @returns An ArrayBuffer with error flag, length prefix, and payload
 */
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

/**
 * Memory class for interacting with persistent collection storage.
 * Provides methods for reading, writing, and deleting collection items.
 */
export class Memory {
  /**
   * Writes a value to the persistent collection storage.
   * 
   * @template V - The type of value to write
   * @param key - The key to write to
   * @param val - The value to write
   */
  static writeCollection<V>(key: string, val: V): void {
    let rawKey = getLengthPrefixedString(Result.Ok<string, WeilError>(key));
    let rawVal = getLengthPrefixedString(Result.Ok<string, WeilError>(JSON.stringify<V>(val)));

    writeCollection(rawKey, rawVal)
  }

  /**
   * Deletes a value from the persistent collection storage.
   * 
   * @template V - The type of value to delete
   * @param key - The key to delete
   * @returns The deleted value, or null if not found
   */
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

  /**
   * Reads a value from the persistent collection storage.
   * 
   * @template V - The type of value to read
   * @param key - The key to read
   * @returns The value, or null if not found
   */
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

/**
 * Runtime class for interacting with the Weil runtime environment.
 * Provides methods for accessing state, arguments, contract information, and making cross-contract calls.
 */
export class Runtime {
  /**
   * Gets the current contract state.
   * 
   * @template T - The type of the state
   * @returns The deserialized contract state
   */
  static state<T>(): T {
    let ptr = getStateAndArgs();
    let memString = readStringFromMemory(ptr);
    let serializedStateAndArgs = memString.tryValue();
    let stateArgs = JSON.parse<StateArgsValue>(serializedStateAndArgs);

    return JSON.parse<T>(stateArgs.state)
  }

  /**
   * Gets the current method arguments.
   * 
   * @template T - The type of the arguments
   * @returns The deserialized method arguments
   */
  static args<T>(): T {
    let ptr = getArgs();
    let serializedArgs = readStringFromMemory(ptr).tryValue();

    return JSON.parse<T>(serializedArgs)
  }

  /**
   * Gets both the contract state and method arguments as a tuple.
   * 
   * @template T - The type of the state
   * @template U - The type of the arguments
   * @returns A Tuple containing the state and arguments
   */
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

  /**
   * Gets the current contract ID.
   * 
   * @returns The contract ID as a string
   */
  static contractId(): string {
    let ptr = getContractId();
    return readStringFromMemory(ptr).tryValue();
  }

  /**
   * Gets the address of the transaction sender.
   * 
   * @returns The sender address as a string
   */
  static sender(): string {
    let ptr = getSender();
    return readStringFromMemory(ptr).tryValue();
  }

  /**
   * Calls a method on another contract.
   * 
   * @template R - The return type of the contract method
   * @param contractId - The ID of the contract to call
   * @param methodName - The name of the method to call
   * @param methodArgs - The serialized method arguments, or null for no arguments
   * @returns A Result containing the return value or an error
   */
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

  /**
   * Gets the ledger contract ID.
   * 
   * @returns The ledger contract ID as a string
   */
  static ledgerContractId(): string {
    let ptr = getLedgerContractId();
    return readStringFromMemory(ptr).tryValue()
  }

  /**
   * Gets the current block height.
   * 
   * @returns The block height as a u32
   */
  static blockHeight(): u32 {
    let ptr = getBlockHeight();
    return <u32>parseInt(readStringFromMemory(ptr).tryValue());
  }

  /**
   * Gets the current block timestamp.
   * 
   * @returns The block timestamp as a string
   */
  static blockTimestamp(): string {
    let ptr = getBlockTimestamp();
    return readStringFromMemory(ptr).tryValue()
  }

  /**
   * Sets the contract state.
   * 
   * @template T - The type of the state
   * @param state - The state to set
   */
  static setState<T>(state: T): void {
    let serializedState = JSON.stringify<T>(state);
    setState(getLengthPrefixedString(Result.Ok<string, WeilError>(serializedState)))
  }

  /**
   * Sets an error result for the contract method.
   * 
   * @template T - The error type (must extend JsonSerializable)
   * @param error - The error to set as the result
   */
  static setErrorResult<T extends JsonSerializable = WeilError>(error: T): void {
    const weilError = error instanceof WeilError
      ? error as WeilError
      : new WeilError("Error");

    Runtime.setStateAndResult<string, string>(Result.Err<WeilValue<string, string>, WeilError>(weilError));
  }

  /**
   * Sets a successful result for the contract method.
   * 
   * @template T - The type of the result
   * @param result - The result value to set
   */
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

  /**
   * Sets the result for the contract method from a Result type.
   * NOTE: This method may panic in some cases.
   * 
   * @template T - The type of the success value
   * @template E - The error type (must extend JsonSerializable)
   * @param result - The Result to set as the method result
   */
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

  /**
   * Sets both the contract state and a successful result.
   * 
   * @template T - The type of the state
   * @template U - The type of the result value
   * @param weilValue - The WeilValue containing state and result
   */
  static setStateAndOkResult<T, U>(weilValue: WeilValue<T, U>): void {
    const rawValue = weilValue.raw();
    const serializedResult = getLengthPrefixedString(
      Result.Ok<string, WeilError>(JSON.stringify(rawValue))
    );
    setStateAndResult(serializedResult);
  }

  /**
   * Sets both the contract state and an error result.
   * 
   * @param error - The error to set as the result
   */
  static setStateAndErrResult(error: WeilError): void {
    const serializedResult = getLengthPrefixedString(
      Result.Err<string, WeilError>(error)
    );
    setStateAndResult(serializedResult);
  }

  /**
   * Sets both the contract state and result from a Result type.
   * 
   * @template T - The type of the state
   * @template U - The type of the result value
   * @param result - The Result containing WeilValue or error
   */
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

  /**
   * Logs a debug message.
   * 
   * @param log - The message to log
   */
  static debugLog(log: string): void {
    debugLog(getLengthPrefixedString(Result.Ok<string, WeilError>(log)))
  }

  /**
   * Allocates a block of memory and pins it to prevent garbage collection.
   * 
   * @param size - The size of memory to allocate in bytes
   * @param id - The type ID for the allocation
   * @returns A pointer to the allocated memory
   */
  static allocate(size: usize, id: u32): usize {
    let ptr = __new(size, id); // Allocate memory block
    __pin(ptr); // Pin the memory to prevent GC
    return ptr; // Return the pointer to the allocated memory
  }

  /**
   * Deallocates memory at a specified pointer by unpinning it.
   * This allows the memory to be garbage collected.
   * 
   * @param ptr - The pointer to the memory to deallocate
   */
  static deallocate(ptr: usize): void {
    __unpin(ptr); // Unpin the memory, allowing it to be garbage collected
  }

}
