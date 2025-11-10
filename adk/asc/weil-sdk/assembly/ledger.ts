import { Runtime } from './runtime'
import { JSON } from 'json-as'
import { WeilMap } from './collections/map'
import { WeilError } from './error'
import { Result } from './result'
import { Box } from './primitives'

/**
 * Arguments for the ledger balance_for method.
 */
@json
class LedgerBalanceMethodArgs {
  /** The address to query */
  addr: string = ''
  /** The token symbol to query */
  symbol: string = ''
}

/**
 * Arguments for the ledger balances_for method.
 */
@json
class LedgerBalancesMethodArgs {
  /** The address to query */
  addr: string = ''
}

/**
 * Arguments for the ledger transfer method.
 */
@json
class LedgerTransferMethodArgs {
  /** The token symbol to transfer */
  symbol: string = ''
  /** The address to transfer from */
  from_addr: string = ''
  /** The address to transfer to */
  to_addr: string = ''
  /** The amount to transfer */
  amount: u64 = 0
}

/**
 * Arguments for the ledger mint method.
 */
@json
class LedgerMintMethodArgs {
  /** The token symbol to mint */
  symbol: string = ''
  /** The address to mint to */
  to_addr: string = ''
  /** The amount to mint */
  amount: u64 = 0
}

/**
 * Ledger class for interacting with the ledger contract.
 * Provides methods for querying balances, transferring tokens, and minting.
 */
export class Ledger {
  /**
   * Fetches all token balances for a given address.
   * 
   * @param addr - The address to query balances for
   * @returns A Result containing a map of token symbols to balances, or an error
   */
  static balancesFor(addr: string): Result<WeilMap<string, u64>, WeilError> {
    // Serialize the method arguments to JSON
    const serializedArgs: string = JSON.stringify<LedgerBalancesMethodArgs>({
      addr,
    })

    // Call contract method and return balances
    const result: Result<
      Box<WeilMap<string, u64>>,
      WeilError
    > = Runtime.callContract<WeilMap<string, u64>>(
      Runtime.ledgerContractId(),
      'balances_for',
      serializedArgs,
    )

    if (result.isOk()) {
      return Result.Ok<WeilMap<string, u64>, WeilError>(result.tryValue().value)
    }

    return Result.Err<WeilMap<string, u64>, WeilError>(result.tryError())
  }

  /**
   * Fetches the balance of a specific token for a given address.
   * 
   * @param addr - The address to query
   * @param symbol - The token symbol to query
   * @returns A Result containing the balance, or an error
   */
  static balanceFor(addr: string, symbol: string): Result<Box<u64>, WeilError> {
    const serializedArgs: string = JSON.stringify<LedgerBalanceMethodArgs>({
      addr,
      symbol,
    })

    // Call contract method and return the balance
    return Runtime.callContract<u64>(
      Runtime.ledgerContractId(),
      'balance_for',
      serializedArgs,
    )
  }

  /**
   * Transfers a token from one address to another.
   * 
   * @param symbol - The token symbol to transfer
   * @param fromAddr - The address to transfer from
   * @param toAddr - The address to transfer to
   * @param amount - The amount to transfer
   * @returns A Result indicating success or failure
   */
  static transfer(
    symbol: string,
    fromAddr: string,
    toAddr: string,
    amount: u64,
  ): Result<Box<i32>, WeilError> {
    // Serialize the method arguments to JSON
    const serializedArgs: string = JSON.stringify<LedgerTransferMethodArgs>({
      symbol,
      from_addr: fromAddr,
      to_addr: toAddr,
      amount,
    })

    // Call contract method to transfer tokens
    return Runtime.callContract<i32>(
      Runtime.ledgerContractId(),
      'transfer',
      serializedArgs,
    )
  }

  /**
   * Mints new tokens to a given address.
   * 
   * @param symbol - The token symbol to mint
   * @param toAddr - The address to mint tokens to
   * @param amount - The amount to mint
   * @returns A Result indicating success or failure
   */
  static mint(
    symbol: string,
    toAddr: string,
    amount: u64
  ): Result<Box<i32>, WeilError>{
    // Serialize the method arguments to JSON
    const serializedArgs: string = JSON.stringify<LedgerMintMethodArgs>({
      symbol,
      to_addr: toAddr,
      amount,
    })

    // Call contract method to transfer tokens
    return Runtime.callContract<i32>(
      Runtime.ledgerContractId(),
      'mint',
      serializedArgs,
    )
  }
}
