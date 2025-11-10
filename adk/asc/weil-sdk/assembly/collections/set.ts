import { WeilId } from './weil-id'
import { Memory } from '../runtime'
import { JSON } from 'json-as'

/** Stub value used internally to represent set membership */
const STUB_VALUE: i32 = -1

/**
 * A persistent set collection that stores unique values in the Weil runtime state.
 * 
 * @template T - The type of values in the set
 */
@json
export class WeilSet<T> {
  /** The unique identifier for this set's state storage */
  stateId: WeilId

  /**
   * Creates a new WeilSet instance with the specified state ID.
   * 
   * @param stateId - The unique identifier for this set's state storage
   */
  constructor(stateId: WeilId) {
    this.stateId = stateId
  }

  /**
   * Gets the base state path for this set.
   * 
   * @returns The string representation of the state ID
   */
  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  /**
   * Generates a unique state tree key for a given value.
   * 
   * @param value - The value to generate a state tree key for
   * @returns A unique string key for state storage
   */
  private getStateTreeKey(value: T): string {
    return [this.getBaseStatePath(), JSON.stringify(value)].join('_')
  }

  /**
   * Adds a value to the set.
   * 
   * @param value - The value to add to the set
   */
  add(value: T): void {
    return Memory.writeCollection<i32>(this.getStateTreeKey(value), STUB_VALUE)
  }

  /**
   * Checks whether a value exists in the set.
   * 
   * @param value - The value to check
   * @returns True if the value exists in the set, false otherwise
   */
  has(value: T): boolean {
    return Memory.readCollection<T>(this.getStateTreeKey(value)) !== null
  }

  /**
   * Removes a value from the set.
   * 
   * @param value - The value to remove from the set
   * @returns The value that was removed, or null if not found
   */
  delete(value: T): T | null {
    return Memory.deleteCollection<T>(this.getStateTreeKey(value))
  }
}
