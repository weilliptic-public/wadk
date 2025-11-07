import { JSON } from 'json-as'
import { JsonSerializable } from './JsonSerializable'

/**
 * A wrapper class that makes any type JSON serializable.
 * 
 * @template T - The type of value to wrap
 */
export class JSONWrapper<T> extends JsonSerializable {
  /** The wrapped value */
  inner: T

  /**
   * Creates a new JSONWrapper instance.
   * 
   * @param inner - The value to wrap
   */
  constructor(inner: T) {
    super()
    this.inner = inner
  }

  /**
   * Serializes the wrapped value to a JSON string.
   * 
   * @returns A JSON string representation of the wrapped value
   */
  toJSON(): string {
    return JSON.stringify<T>(this.inner)
  }
}
