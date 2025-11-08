import * as asJSON from 'assemblyscript-json/assembly/JSON'
import { JSON as JSONas } from 'json-as'
import { JsonSerializable } from '../json/JsonSerializable'

/**
 * Error class for token details fetch operations.
 * Represents various error conditions when fetching token details.
 */
export class DetailsFetchError extends JsonSerializable {
  /** The type of error */
  type: string
  /** The token ID that caused the error */
  tokenId: string

  /**
   * Creates a new DetailsFetchError instance.
   * 
   * @param type - The type of error
   * @param tokenId - The token ID that caused the error
   */
  constructor(type: string, tokenId: string) {
    super()
    this.type = type
    this.tokenId = tokenId
  }

  /**
   * Creates an error for an invalid token ID.
   * 
   * @param tokenId - The invalid token ID
   * @returns A DetailsFetchError instance
   */
  static InvalidTokenId(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('InvalidTokenId', tokenId)
  }

  /**
   * Creates an error for a token that has not been minted.
   * 
   * @param tokenId - The token ID that hasn't been minted
   * @returns A DetailsFetchError instance
   */
  static TokenNotMinted(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('TokenNotMinted', tokenId)
  }

  /**
   * Creates an error for a token that was not found.
   * 
   * @param tokenId - The token ID that was not found
   * @returns A DetailsFetchError instance
   */
  static TokenNotFound(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('TokenNotFound', tokenId)
  }

  /**
   * Deserializes a DetailsFetchError from a JSON string.
   * 
   * @param s - The JSON string to parse
   * @returns A DetailsFetchError instance
   */
  static fromJSON(s: string): DetailsFetchError {
    const json: asJSON.Obj = <asJSON.Obj>asJSON.parse(s)
    const type: string = json.keys[0]
    const details = json.getValue(type)
    const tokenId: string = (<asJSON.Str>details).toString()
    return new DetailsFetchError(type, tokenId)
  }

  /**
   * Serializes the error to a JSON string.
   * 
   * @returns A JSON string representation of the error
   */
  toJSON(): string {
    const map: Map<string, string> = new Map<string, string>()
    map.set(this.type, this.tokenId)
    return JSONas.stringify<Map<string, string>>(map)
  }
}
