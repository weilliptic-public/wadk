@json
export class CounterContractState {
  counter: u64

  constructor() {
    this.counter = 0
  }

  increment(): void {
    this.counter += 1
  }

  getCount(): u64 {
    return this.counter
  }

  setValue(value: u64): void {
    this.counter = value
  }
}

@json
export class SetValueArgs {
  val: u64

  constructor() {
    this.val = 0
  }
}
