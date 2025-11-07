import { FungibleToken } from '@weilliptic/weil-sdk-asc/assembly/contracts/fungible'
import { Result } from '@weilliptic/weil-sdk-asc/assembly/result'
import { Box } from '@weilliptic/weil-sdk-asc/assembly/primitives'
import { WeilError } from '@weilliptic/weil-sdk-asc/assembly/error'
import { Tuple } from '@weilliptic/weil-sdk-asc/assembly/json/tuples'
import { JsonSerializable } from '@weilliptic/weil-sdk-asc/assembly/json/JsonSerializable'

class JsonString extends JsonSerializable {
  constructor(private value: string) {
    super();
  }
  toJSON(): string {
    return this.value;
  }
}

class JsonInt extends JsonSerializable {
  constructor(private value: i32) {
    super();
  }
  toJSON(): string {
    return this.value.toString();
  }
}

@json
export class YutakaContractState {
  inner: FungibleToken

  constructor() {
    let totalSupply: u64 = 100000000000

    this.inner = new FungibleToken('Yutaka', 'YTK')
    this.inner.mint(totalSupply)
  }

  name(): string {
    return this.inner.name
  }

  symbol(): string {
    return this.inner.symbol
  }

  decimals(): u8 {
    return 6
  }

  details(): Tuple {
    return new Tuple([
      new JsonString(this.name()),
      new JsonString(this.symbol()),
      new JsonInt(this.decimals())
    ]);
  }

  totalSupply(): u64 {
    return this.inner.totalSupply
  }

  balanceFor(addr: string): Result<Box<u64>, WeilError> {
    return this.inner.balanceFor(addr)
  }

  transfer(to_addr: string, amount: u64): Result<Box<i32>, WeilError> {
    return this.inner.transfer(to_addr, amount)
  }

  approve(spender: string, amount: u64): void {
    this.inner.approve(spender, amount)
  }

  transferFrom(
    from_addr: string,
    to_addr: string,
    amount: u64,
  ): Result<Box<i32>, WeilError> {
    return this.inner.transferFrom(from_addr, to_addr, amount)
  }

  allowance(owner: string, spender: string): u64 {
    return this.inner.allowance(owner, spender)
  }
}
