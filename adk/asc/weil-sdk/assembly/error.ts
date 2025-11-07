import * as asJSON from 'assemblyscript-json/assembly/JSON'
import { JSON, JSON as JSONas } from 'json-as'
import { JsonSerializable } from './json/JsonSerializable'

/**
 * Error information for method execution failures.
 */
@json
export class MethodError {
  /** The name of the method that failed */
  method_name: string = ''
  /** The error message */
  err_msg: string = ''
}

/**
 * Error information for contract call failures.
 */
@json
export class ContractCallError {
  /** The contract ID where the error occurred */
  contract_id: string = ''
  /** The name of the method that failed */
  method_name: string = ''
  /** The error message */
  err_msg: string = ''
}

/**
 * A simple error class that wraps a string message.
 */
export class StringError extends JsonSerializable {
  /** The error message */
  message: string

  /**
   * Creates a new StringError instance.
   * 
   * @param message - The error message
   */
  constructor(message: string) {
    super()
    this.message = message
  }

  /**
   * Serializes the error to a JSON string.
   * 
   * @returns A JSON string representation of the error message
   */
  toJSON(): string {
    return JSON.stringify<string>(this.message)
  }
}

/**
 * Main error class for Weil runtime operations.
 * Can represent various types of errors with different error details.
 */
export class WeilError extends JsonSerializable {
  /** The type of error */
  type: string
  /** String-based error message, if applicable */
  stringError: string | null
  /** Method error details, if applicable */
  methodError: MethodError | null
  /** Contract call error details, if applicable */
  contractCallError: ContractCallError | null

  /**
   * Creates a new WeilError instance.
   * 
   * @param type - The type of error
   */
  constructor(type: string) {
    super()
    this.type = type
    this.stringError = null
    this.methodError = null
    this.contractCallError = null
  }

  /**
   * Creates an error for method argument deserialization failures.
   * 
   * @param error - The method error details
   * @returns A WeilError instance
   */
  static MethodArgumentDeserializationError(error: MethodError): WeilError {
    const result = new WeilError('MethodArgumentDeserializationError')
    result.methodError = error
    return result
  }

  /**
   * Creates an error for when a function returns with an error.
   * 
   * @param error - The method error details
   * @returns A WeilError instance
   */
  static FunctionReturnedWithError(error: MethodError): WeilError {
    const result = new WeilError('FunctionReturnedWithError')
    result.methodError = error
    return result
  }

  /**
   * Creates an error for WASM module execution traps.
   * 
   * @param error - The method error details
   * @returns A WeilError instance
   */
  static TrapOccuredWhileWasmModuleExecution(error: MethodError): WeilError {
    const result = new WeilError('TrapOccuredWhileWasmModuleExecution')
    result.methodError = error
    return result
  }

  /**
   * Creates an error for when a key is not found in a collection.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static KeyNotFoundInCollection(error: string): WeilError {
    const result = new WeilError('KeyNotFoundInCollection')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for when no value is returned from deleting a collection item.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static NoValueReturnedFromDeletingCollectionItem(error: string): WeilError {
    const result = new WeilError('NoValueReturnedFromDeletingCollectionItem')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for when entries are not found in a collection for keys with a prefix.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static EntriesNotFoundInCollectionForKeysWithPrefix(
    error: string,
  ): WeilError {
    const result = new WeilError('EntriesNotFoundInCollectionForKeysWithPrefix')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for contract method execution failures.
   * 
   * @param error - The contract call error details
   * @returns A WeilError instance
   */
  static ContractMethodExecutionError(error: ContractCallError): WeilError {
    const result = new WeilError('ContractMethodExecutionError')
    result.contractCallError = error
    return result
  }

  /**
   * Creates an error for invalid cross-contract calls.
   * 
   * @param error - The contract call error details
   * @returns A WeilError instance
   */
  static InvalidCrossContractCallError(error: ContractCallError): WeilError {
    const result = new WeilError('InvalidCrossContractCallError')
    result.contractCallError = error
    return result
  }

  /**
   * Creates an error for cross-contract call result deserialization failures.
   * 
   * @param error - The contract call error details
   * @returns A WeilError instance
   */
  static CrossContractCallResultDeserializationError(
    error: ContractCallError,
  ): WeilError {
    const result = new WeilError('CrossContractCallResultDeserializationError')
    result.contractCallError = error
    return result
  }

  /**
   * Creates an error for streaming response deserialization failures.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static StreamingResponseDeserializationError(error: string): WeilError {
    const result = new WeilError('StreamingResponseDeserializationError')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for invalid data received.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static InvalidDataReceivedError(error: string): WeilError {
    const result = new WeilError('InvalidDataReceivedError')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for outcall failures.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static OutcallError(error: string): WeilError {
    const result = new WeilError('OutcallError')
    result.stringError = error
    return result
  }

  /**
   * Creates an error for invalid WASM module errors.
   * 
   * @param error - The error message
   * @returns A WeilError instance
   */
  static InvalidWasmModuleError(error: string): WeilError {
    const result = new WeilError('InvalidWasmModuleError')
    result.stringError = error
    return result
  }

  /**
   * Deserializes a WeilError from a JSON string.
   * 
   * @param s - The JSON string to parse
   * @returns A WeilError instance
   */
  static fromJSON(s: string): WeilError {
    const json: asJSON.Obj = <asJSON.Obj>asJSON.parse(s)
    const type: string = json.keys[0]
    const details = json.getValue(type)
    const weilError = new WeilError(type)

    if (details) {
      const detailsAsString = details.stringify()

      if (
        type === 'MethodArgumentDeserializationError' ||
        type === 'TrapOccuredWhileWasmModuleExecution'
      ) {
        weilError.methodError = JSONas.parse<MethodError>(detailsAsString)
      } else if (
        type === 'KeyNotFoundInCollection' ||
        type === 'NoValueReturnedFromDeletingCollectionItem' ||
        type === 'EntriesNotFoundInCollectionForKeysWithPrefix' ||
        type === 'OutcallError' ||
        type === 'InvalidDataReceivedError' ||
        type === 'InvalidWasmModuleError' ||
        type === 'StreamingResponseDeserializationError'
      ) {
        weilError.stringError = detailsAsString
      } else if (
        type === 'ContractMethodExecutionError' ||
        type === 'InvalidCrossContractCallError' ||
        type === 'CrossContractCallResultDeserializationError'
      ) {
        weilError.contractCallError =
          JSONas.parse<ContractCallError>(detailsAsString)
      }
    }

    return weilError
  }

  /**
   * Serializes the error to a JSON string.
   * 
   * @returns A JSON string representation of the error
   */
  toJSON(): string {
    if (this.stringError) {
      const map: Map<string, string> = new Map<string, string>()
      map.set(this.type, <string>this.stringError)
      return JSONas.stringify<Map<string, string>>(map)
    }
    if (this.methodError) {
      const map: Map<string, MethodError> = new Map<string, MethodError>()
      map.set(this.type, <MethodError>this.methodError)
      return JSONas.stringify<Map<string, MethodError>>(map)
    }
    if (this.contractCallError) {
      const map: Map<string, ContractCallError> = new Map<
        string,
        ContractCallError
      >()
      map.set(this.type, <ContractCallError>this.contractCallError)
      return JSONas.stringify<Map<string, ContractCallError>>(map)
    }

    return ''
  }
}

/**
 * Error thrown when an index is out of bounds for array/vector operations.
 */
export class IndexOutOfBoundsError extends Error {
  /** The error name */
  name: 'IndexOutOfBoundsError'
  
  /**
   * Creates a new IndexOutOfBoundsError instance.
   * 
   * @param message - The error message
   */
  constructor(message: string) {
    super()
    this.name = 'IndexOutOfBoundsError'
    this.message = message
  }
}
