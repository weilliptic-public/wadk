import { Runtime } from './runtime'
import { JSON } from 'json-as'
import { WeilMap } from './collections/map'
import { WeilError } from './error'
import { Result } from './result'
import { Box } from './primitives'

@json
class LedgerBalanceMethodArgs {
  addr: string = ''
  symbol: string = ''
}

@json
class LedgerBalancesMethodArgs {
  addr: string = ''
}

@json
class LedgerTransferMethodArgs {
  symbol: string = ''
  from_addr: string = ''
  to_addr: string = ''
  amount: u64 = 0
}

@json
class LedgerMintMethodArgs {
  symbol: string = ''
  to_addr: string = ''
  amount: u64 = 0
}

// Ledger class equivalent to the Rust struct
export class Ledger {
  // Fetches all balances for a given address
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

  // Fetches the balance of a specific token for a given address
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

  // Transfers a token from one address to another
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

  // Mints
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
