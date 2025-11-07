import { WeilId } from './weil-id'
import { Memory } from '../runtime'
import { JSON } from 'json-as'

/**
 * A persistent key-value map collection that stores data in the Weil runtime state.
 * 
 * @template K - The type of keys in the map
 * @template V - The type of values in the map
 */
@json
export class WeilMap<K, V> {
  /** The unique identifier for this map's state storage */
  stateId: WeilId

  /**
   * Creates a new WeilMap instance with the specified state ID.
   * 
   * @param stateId - The unique identifier for this map's state storage
   */
  constructor(stateId: WeilId) {
    this.stateId = stateId
  }

  /**
   * Gets the base state path for this map.
   * 
   * @returns The string representation of the state ID
   */
  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  /**
   * Generates a unique state tree key for a given key suffix.
   * 
   * @param suffix - The key to generate a state tree key for
   * @returns A unique string key for state storage
   */
  private getStateTreeKey(suffix: K): string {
    return [this.getBaseStatePath(), JSON.stringify(suffix)].join('_')
  }

  /**
   * Retrieves the value associated with the given key.
   * 
   * @param key - The key to look up
   * @returns The value associated with the key, or null if not found
   */
  get(key: K): V | null {
    return Memory.readCollection<V>(this.getStateTreeKey(key))
  }

  /**
   * Checks whether a key exists in the map.
   * 
   * @param key - The key to check
   * @returns True if the key exists, false otherwise
   */
  has(key: K): boolean {
    return this.get(key) !== null
  }

  /**
   * Sets a key-value pair in the map.
   * 
   * @param key - The key to set
   * @param value - The value to associate with the key
   */
  set(key: K, value: V): void {
    return Memory.writeCollection<V>(this.getStateTreeKey(key), value)
  }

  /**
   * Deletes a key-value pair from the map.
   * 
   * @param key - The key to delete
   * @returns The value that was associated with the key, or null if not found
   */
  delete(key: K): V | null {
    return Memory.deleteCollection<V>(this.getStateTreeKey(key))
  }
}
