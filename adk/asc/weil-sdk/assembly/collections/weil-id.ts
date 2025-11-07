/**
 * A unique identifier for Weil state storage collections.
 * Used to namespace different collections in the persistent state.
 */
@json
export class WeilId {
  /** The numeric identifier */
  id: u8

  /**
   * Creates a new WeilId instance with the specified ID.
   * 
   * @param id - The numeric identifier (0-255)
   */
  constructor(id: u8) {
    this.id = id
  }

  /**
   * Converts the ID to its string representation.
   * 
   * @returns The string representation of the ID
   */
  toString(): string {
    return this.id.toString()
  }
}
