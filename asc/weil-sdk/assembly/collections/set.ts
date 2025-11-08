import { WeilId } from './weil-id'
import { Memory } from '../runtime'
import { JSON } from 'json-as'

const STUB_VALUE: i32 = -1

@json
export class WeilSet<T> {
  stateId: WeilId

  constructor(stateId: WeilId) {
    this.stateId = stateId
  }

  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  private getStateTreeKey(value: T): string {
    return [this.getBaseStatePath(), JSON.stringify(value)].join('_')
  }

  add(value: T): void {
    return Memory.writeCollection<i32>(this.getStateTreeKey(value), STUB_VALUE)
  }

  has(value: T): boolean {
    return Memory.readCollection<T>(this.getStateTreeKey(value)) !== null
  }

  delete(value: T): T | null {
    return Memory.deleteCollection<T>(this.getStateTreeKey(value))
  }
}
