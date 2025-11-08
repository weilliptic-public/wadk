import { JSON } from 'json-as'
import { JsonSerializable } from './JsonSerializable'

export class JSONWrapper<T> extends JsonSerializable {
  inner: T

  constructor(inner: T) {
    super()
    this.inner = inner
  }

  toJSON(): string {
    return JSON.stringify<T>(this.inner)
  }
}
