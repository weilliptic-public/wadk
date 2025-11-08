@json
export class WeilId {
  id: u8

  constructor(id: u8) {
    this.id = id
  }

  toString(): string {
    return this.id.toString()
  }
}
