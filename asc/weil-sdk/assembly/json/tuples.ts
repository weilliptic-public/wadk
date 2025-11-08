import { JsonSerializable } from './JsonSerializable'

/**
 * A tuple class that holds an array of JSON-serializable elements.
 * Serializes to a JSON array format.
 */
export class Tuple extends JsonSerializable {
  /** The array of JSON-serializable elements */
  elements: JsonSerializable[]

  /**
   * Creates a new Tuple instance.
   * 
   * @param elements - An array of JSON-serializable elements
   */
  constructor(elements: JsonSerializable[]) {
    super()
    this.elements = elements
  }

  /**
   * Serializes the tuple to a JSON array string.
   * 
   * @returns A JSON array string representation of the tuple elements
   */
  toJSON(): string {
    const args = this.elements
      .map((element: JsonSerializable) => element.toJSON())
      .join(',')

    return ['[', args, ']'].join('')
  }
}
