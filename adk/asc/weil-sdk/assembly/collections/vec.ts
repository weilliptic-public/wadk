import { WeilId } from './weil-id'
import { IndexOutOfBoundsError, WeilError } from '../error'
import { Memory } from '../runtime'

/**
 * A persistent vector collection that stores ordered elements in the Weil runtime state.
 * 
 * @template T - The type of elements in the vector
 */
export class WeilVec<T> {
  /** The unique identifier for this vector's state storage */
  stateId: WeilId
  /** The current length of the vector */
  length: usize

  /**
   * Creates a new WeilVec instance with the specified state ID.
   * 
   * @param stateId - The unique identifier for this vector's state storage
   */
  constructor(stateId: WeilId) {
    this.stateId = stateId
    this.length = 0
  }

  /**
   * Gets the base state path for this vector.
   * 
   * @returns The string representation of the state ID
   */
  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  /**
   * Generates a unique state tree key for a given index.
   * 
   * @param suffix - The index to generate a state tree key for
   * @returns A unique string key for state storage
   */
  private getStateTreeKey(suffix: usize): string {
    return [this.getBaseStatePath(), suffix.toString()].join('_')
  }

  /**
   * Appends an item to the end of the vector.
   * 
   * @param item - The item to append
   */
  push(item: T): void {
    Memory.writeCollection(this.getStateTreeKey(this.length), item)
    this.length += 1
  }

  /**
   * Retrieves the element at the specified index.
   * 
   * @param index - The index of the element to retrieve
   * @returns The element at the index, or null if not found
   */
  get(index: usize): T | null {
    return Memory.readCollection<T>(this.getStateTreeKey(index))
  }

  /**
   * Sets the element at the specified index.
   * 
   * @param index - The index at which to set the element
   * @param item - The item to set at the index
   * @throws {IndexOutOfBoundsError} If the index is out of bounds
   */
  set(index: usize, item: T): void {
    if (index >= this.length) {
      throw new IndexOutOfBoundsError(
        `Index out of bounds: ${index}. Vector length is ${this.length}.`,
      )
    }

    Memory.writeCollection(this.getStateTreeKey(this.length), item)
  }

  /**
   * Removes and returns the last element of the vector.
   * 
   * @returns The last element, or null if the vector is empty
   */
  pop(): T | null {
    if (this.length === 0) {
      return null
    }

    this.length -= 1

    return Memory.deleteCollection<T>(this.getStateTreeKey(this.length))
  }

  /**
   * Executes a callback function for each element in the vector.
   * 
   * @param callbackfn - A function to execute for each element, receiving the value, index, and vector
   * @throws {WeilError} If an element at an index is not set
   */
  forEach(callbackfn: (value: T, index: i32, vec: WeilVec<T>) => void): void {
    for (let i: i32 = 0; i < this.length; i++) {
      const item = this.get(i)
      if (item === null) {
        throw new WeilError(`WeilVec item at index ${i} is not set`)
      } else {
        callbackfn(item, i, this)
      }
    }
  }
}
