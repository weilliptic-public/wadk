import * as asJSON from 'assemblyscript-json/assembly/JSON'
import { JSON, JSON as JSONas } from 'json-as'
import { JsonSerializable } from './json/JsonSerializable'

@json
export class MethodError {
  method_name: string = ''
  err_msg: string = ''
}

@json
export class ContractCallError {
  contract_id: string = ''
  method_name: string = ''
  err_msg: string = ''
}

export class StringError extends JsonSerializable {
  message: string

  constructor(message: string) {
    super()
    this.message = message
  }

  toJSON(): string {
    return JSON.stringify<string>(this.message)
  }
}

export class WeilError extends JsonSerializable {
  type: string
  stringError: string | null
  methodError: MethodError | null
  contractCallError: ContractCallError | null

  constructor(type: string) {
    super()
    this.type = type
    this.stringError = null
    this.methodError = null
    this.contractCallError = null
  }

  static MethodArgumentDeserializationError(error: MethodError): WeilError {
    const result = new WeilError('MethodArgumentDeserializationError')
    result.methodError = error
    return result
  }

  static FunctionReturnedWithError(error: MethodError): WeilError {
    const result = new WeilError('FunctionReturnedWithError')
    result.methodError = error
    return result
  }

  static TrapOccuredWhileWasmModuleExecution(error: MethodError): WeilError {
    const result = new WeilError('TrapOccuredWhileWasmModuleExecution')
    result.methodError = error
    return result
  }

  static KeyNotFoundInCollection(error: string): WeilError {
    const result = new WeilError('KeyNotFoundInCollection')
    result.stringError = error
    return result
  }

  static NoValueReturnedFromDeletingCollectionItem(error: string): WeilError {
    const result = new WeilError('NoValueReturnedFromDeletingCollectionItem')
    result.stringError = error
    return result
  }

  static EntriesNotFoundInCollectionForKeysWithPrefix(
    error: string,
  ): WeilError {
    const result = new WeilError('EntriesNotFoundInCollectionForKeysWithPrefix')
    result.stringError = error
    return result
  }

  static ContractMethodExecutionError(error: ContractCallError): WeilError {
    const result = new WeilError('ContractMethodExecutionError')
    result.contractCallError = error
    return result
  }

  static InvalidCrossContractCallError(error: ContractCallError): WeilError {
    const result = new WeilError('InvalidCrossContractCallError')
    result.contractCallError = error
    return result
  }

  static CrossContractCallResultDeserializationError(
    error: ContractCallError,
  ): WeilError {
    const result = new WeilError('CrossContractCallResultDeserializationError')
    result.contractCallError = error
    return result
  }

  static StreamingResponseDeserializationError(error: string): WeilError {
    const result = new WeilError('StreamingResponseDeserializationError')
    result.stringError = error
    return result
  }

  static InvalidDataReceivedError(error: string): WeilError {
    const result = new WeilError('InvalidDataReceivedError')
    result.stringError = error
    return result
  }

  static OutcallError(error: string): WeilError {
    const result = new WeilError('OutcallError')
    result.stringError = error
    return result
  }

  static InvalidWasmModuleError(error: string): WeilError {
    const result = new WeilError('InvalidWasmModuleError')
    result.stringError = error
    return result
  }

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

export class IndexOutOfBoundsError extends Error {
  name: 'IndexOutOfBoundsError'
  constructor(message: string) {
    super()
    this.name = 'IndexOutOfBoundsError'
    this.message = message
  }
}
