import { Ledger } from '../ledger'
import { WeilMap } from '../collections/map'
import { Runtime } from '../runtime'
import { Result } from '../result'
import { WeilError } from '../error'
import { Box } from '../primitives'
import { WeilId } from '../collections/weil-id'
import { JSON } from 'json-as'
import { Tuple } from '../json/tuples'
import { JSONWrapper } from '../json/primitives'

@json
export class BalanceForArgs {
  addr!: string
}

@json
export class TransferArgs {
  to_addr!: string
  amount!: u64
}

@json
export class ApproveArgs {
  spender!: string
  value!: u64
}

@json
export class TransferFromArgs {
  from_addr!: string
  to_addr!: string
  value!: u64
}

@json
export class AllowanceArgs {
  owner!: string
  spender!: string
}

const getAllowanceKey = (owner: string, spender: string): string =>
  [owner, spender].join('$')

@json
export class FungibleToken {
  name: string
  symbol: string
  totalSupply: u64
  allowances: WeilMap<string, Box<u64>>

  constructor(name: string, symbol: string) {
    this.name = name
    this.symbol = symbol
    this.totalSupply = 0
    this.allowances = new WeilMap<string, Box<u64>>(new WeilId(0))
  }

  balanceFor(addr: string): Result<Box<u64>, WeilError> {
    return Ledger.balanceFor(addr, this.symbol)
  }

  transfer(to_addr: string, amount: u64): Result<Box<i32>, WeilError> {
    return Ledger.transfer(this.symbol, Runtime.sender(), to_addr, amount)
  }

  approve(spender: string, amount: u64): void {
    const key = getAllowanceKey(Runtime.sender(), spender)
    this.allowances.set(key, new Box(amount))
  }

  mint(amount: u64): Result<Box<i32>, WeilError> {
    this.totalSupply+=amount;

    return Ledger.mint(
      this.symbol,
      Runtime.sender(),
      amount
    )
  }

  transferFrom(
    from_addr: string,
    to_addr: string,
    amount: u64,
  ): Result<Box<i32>, WeilError> {
    const key = getAllowanceKey(from_addr, Runtime.sender())
    let balance: u64 = 0
    if (this.allowances.has(key)) {
      balance = this.allowances.get(key)!.value
    }

    if (balance < amount) {
      return Result.Err<Box<i32>, WeilError>(
        WeilError.FunctionReturnedWithError({
          method_name: 'transfer_from',
          err_msg: `Allowance balance of sender ${Runtime.sender()} is ${balance}, which is less than the transfer request amount from ${from_addr}.`,
        }),
      )
    }

    const result = Ledger.transfer(this.symbol, from_addr, to_addr, amount)
    this.allowances.set(key, new Box(balance - amount))

    return result
  }

  allowance(owner: string, spender: string): u64 {
    const key = getAllowanceKey(owner, spender)
    return this.allowances.has(key) ? this.allowances.get(key)!.value : 0
  }
}
