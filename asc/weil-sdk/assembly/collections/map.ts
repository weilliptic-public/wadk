import { WeilId } from './weil-id'
import { Memory } from '../runtime'
import { JSON } from 'json-as'

@json
export class WeilMap<K, V> {
  stateId: WeilId

  constructor(stateId: WeilId) {
    this.stateId = stateId
  }

  private getBaseStatePath(): string {
    return this.stateId.toString()
  }

  private getStateTreeKey(suffix: K): string {
    return [this.getBaseStatePath(), JSON.stringify(suffix)].join('_')
  }

  get(key: K): V | null {
    return Memory.readCollection<V>(this.getStateTreeKey(key))
  }

  has(key: K): boolean {
    return this.get(key) !== null
  }

  set(key: K, value: V): void {
    return Memory.writeCollection<V>(this.getStateTreeKey(key), value)
  }

  delete(key: K): V | null {
    return Memory.deleteCollection<V>(this.getStateTreeKey(key))
  }
}
