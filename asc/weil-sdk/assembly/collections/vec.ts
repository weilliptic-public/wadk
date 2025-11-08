import { WeilId } from './weil-id'
import { IndexOutOfBoundsError, WeilError } from '../error'
import { Memory } from '../runtime'

export class WeilVec<T> {
  stateId: WeilId
  length: usize

  constructor(stateId: WeilId) {
    this.stateId = stateId
    this.length = 0
  }

  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  private getStateTreeKey(suffix: usize): string {
    return [this.getBaseStatePath(), suffix.toString()].join('_')
  }

  push(item: T): void {
    Memory.writeCollection(this.getStateTreeKey(this.length), item)
    this.length += 1
  }

  get(index: usize): T | null {
    return Memory.readCollection<T>(this.getStateTreeKey(index))
  }

  set(index: usize, item: T): void {
    if (index >= this.length) {
      throw new IndexOutOfBoundsError(
        `Index out of bounds: ${index}. Vector length is ${this.length}.`,
      )
    }

    Memory.writeCollection(this.getStateTreeKey(this.length), item)
  }

  pop(): T | null {
    if (this.length === 0) {
      return null
    }

    this.length -= 1

    return Memory.deleteCollection<T>(this.getStateTreeKey(this.length))
  }

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
