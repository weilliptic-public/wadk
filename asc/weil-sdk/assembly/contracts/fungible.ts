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

/**
 * Arguments for the balanceFor method.
 */
@json
export class BalanceForArgs {
  /** The address to query */
  addr!: string
}

/**
 * Arguments for the transfer method.
 */
@json
export class TransferArgs {
  /** The address to transfer to */
  to_addr!: string
  /** The amount to transfer */
  amount!: u64
}

/**
 * Arguments for the approve method.
 */
@json
export class ApproveArgs {
  /** The address to approve as a spender */
  spender!: string
  /** The amount to approve */
  value!: u64
}

/**
 * Arguments for the transferFrom method.
 */
@json
export class TransferFromArgs {
  /** The address to transfer from */
  from_addr!: string
  /** The address to transfer to */
  to_addr!: string
  /** The amount to transfer */
  value!: u64
}

/**
 * Arguments for the allowance method.
 */
@json
export class AllowanceArgs {
  /** The owner address */
  owner!: string
  /** The spender address */
  spender!: string
}

/**
 * Generates a unique key for an allowance mapping.
 * 
 * @param owner - The owner address
 * @param spender - The spender address
 * @returns A unique key string
 */
const getAllowanceKey = (owner: string, spender: string): string =>
  [owner, spender].join('$')

/**
 * FungibleToken class implementing ERC-20-like functionality.
 * Manages token balances, transfers, approvals, and allowances.
 */
@json
export class FungibleToken {
  /** The name of the token */
  name: string
  /** The symbol of the token */
  symbol: string
  /** The total supply of tokens */
  totalSupply: u64
  /** Map of allowance keys to approved amounts */
  allowances: WeilMap<string, Box<u64>>

  /**
   * Creates a new FungibleToken instance.
   * 
   * @param name - The name of the token
   * @param symbol - The symbol of the token
   */
  constructor(name: string, symbol: string) {
    this.name = name
    this.symbol = symbol
    this.totalSupply = 0
    this.allowances = new WeilMap<string, Box<u64>>(new WeilId(0))
  }

  /**
   * Gets the balance of a specific address for this token.
   * 
   * @param addr - The address to query
   * @returns A Result containing the balance or an error
   */
  balanceFor(addr: string): Result<Box<u64>, WeilError> {
    return Ledger.balanceFor(addr, this.symbol)
  }

  /**
   * Transfers tokens from the sender to another address.
   * 
   * @param to_addr - The address to transfer to
   * @param amount - The amount to transfer
   * @returns A Result indicating success or failure
   */
  transfer(to_addr: string, amount: u64): Result<Box<i32>, WeilError> {
    return Ledger.transfer(this.symbol, Runtime.sender(), to_addr, amount)
  }

  /**
   * Approves a spender to transfer tokens on behalf of the sender.
   * 
   * @param spender - The address to approve as a spender
   * @param amount - The amount to approve
   */
  approve(spender: string, amount: u64): void {
    const key = getAllowanceKey(Runtime.sender(), spender)
    this.allowances.set(key, new Box(amount))
  }

  /**
   * Mints new tokens to the sender's address.
   * 
   * @param amount - The amount to mint
   * @returns A Result indicating success or failure
   */
  mint(amount: u64): Result<Box<i32>, WeilError> {
    this.totalSupply+=amount;

    return Ledger.mint(
      this.symbol,
      Runtime.sender(),
      amount
    )
  }

  /**
   * Transfers tokens from one address to another on behalf of the sender.
   * Requires the sender to have sufficient allowance from the from_addr.
   * 
   * @param from_addr - The address to transfer from
   * @param to_addr - The address to transfer to
   * @param amount - The amount to transfer
   * @returns A Result indicating success or failure
   */
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

  /**
   * Gets the allowance amount that a spender is approved to spend on behalf of an owner.
   * 
   * @param owner - The owner address
   * @param spender - The spender address
   * @returns The allowance amount, or 0 if no allowance exists
   */
  allowance(owner: string, spender: string): u64 {
    const key = getAllowanceKey(owner, spender)
    return this.allowances.has(key) ? this.allowances.get(key)!.value : 0
  }
}
