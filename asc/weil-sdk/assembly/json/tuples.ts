import { JsonSerializable } from './JsonSerializable'

export class Tuple extends JsonSerializable {
  elements: JsonSerializable[]

  constructor(elements: JsonSerializable[]) {
    super()
    this.elements = elements
  }

  toJSON(): string {
    const args = this.elements
      .map((element: JsonSerializable) => element.toJSON())
      .join(',')

    return ['[', args, ']'].join('')
  }
}
