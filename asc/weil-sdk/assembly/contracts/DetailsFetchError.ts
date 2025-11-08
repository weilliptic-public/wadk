import * as asJSON from 'assemblyscript-json/assembly/JSON'
import { JSON as JSONas } from 'json-as'
import { JsonSerializable } from '../json/JsonSerializable'

export class DetailsFetchError extends JsonSerializable {
  type: string
  tokenId: string

  constructor(type: string, tokenId: string) {
    super()
    this.type = type
    this.tokenId = tokenId
  }

  static InvalidTokenId(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('InvalidTokenId', tokenId)
  }

  static TokenNotMinted(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('TokenNotMinted', tokenId)
  }

  static TokenNotFound(tokenId: string): DetailsFetchError {
    return new DetailsFetchError('TokenNotFound', tokenId)
  }

  static fromJSON(s: string): DetailsFetchError {
    const json: asJSON.Obj = <asJSON.Obj>asJSON.parse(s)
    const type: string = json.keys[0]
    const details = json.getValue(type)
    const tokenId: string = (<asJSON.Str>details).toString()
    return new DetailsFetchError(type, tokenId)
  }

  toJSON(): string {
    const map: Map<string, string> = new Map<string, string>()
    map.set(this.type, this.tokenId)
    return JSONas.stringify<Map<string, string>>(map)
  }
}
