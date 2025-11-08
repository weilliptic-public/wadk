import { WeilMap } from '../collections/map'
import { WeilId } from '../collections/weil-id'
import { Result } from '../result'
import { DetailsFetchError } from './DetailsFetchError'
import { Runtime } from '../runtime'
import { StringError } from '../error'
import { Ledger } from '../ledger'

@json
class EsSet<K> {
  inner: K[]

  get size(): i32 {
    return this.inner.length
  }

  has(value: K): bool {
    return this.inner.includes(value)
  }

  add(value: K): this {
    if (!this.has(value)) {
      this.inner.push(value)
    }

    return this
  }

  delete(value: K): boolean {
    if (this.has(value)) {
      const newInner: K[] = []
      for (let i = 0; i < this.inner.length; i++) {
        if (this.inner[i] !== value) {
          newInner.push(this.inner[i])
        }
      }

      this.inner = newInner
      return true
    }

    return false
  }

  clear(): void {
    this.inner.length = 0
  }
  values(): K[] {
    return [...this.inner]
  }

  toString(): string {
    return this.inner.toString()
  }

  constructor() {
    this.inner = []
  }
}

@json
export class Token {
  title: string
  name: string
  description: string
  payload: string

  constructor(
    title: string,
    name: string,
    description: string,
    payload: string,
  ) {
    this.title = title
    this.name = name
    this.description = description
    this.payload = payload
  }
}

export type TokenId = string
const EMPTY_TOKEN_ID: string = ''
type Address = string
const EMPTY_ADDRESS: string = ''

@json
export class BalanceOfArgs {
  addr!: string
}

@json
export class TokenIdArgs {
  token_id!: string | null
}
@json
export class ApproveArgs {
  spender!: string | null
  token_id!: string | null
}

@json
export class SetApproveForAllArgs {
  spender!: string | null
  approval!: boolean
}

@json
export class IsApprovedForAllArgs {
  owner!: string | null
  spender!: string | null
}

@json
export class TransferArgs {
  to_addr!: string | null
  token_id!: string | null
}

@json
export class TransferFromArgs {
  from_addr!: string | null
  to_addr!: string | null
  token_id!: string | null
}

@json
export class MintArgs {
  token_id!: string | null
  title!: string | null
  name!: string | null
  description!: string | null
  payload!: string | null
}

const getAllowanceKey = (fromAddr: string, tokenId: string): string =>
  [fromAddr, tokenId].join('$')

@json
export class NonFungibleToken {
  name: string
  creator: Address
  tokens: WeilMap<TokenId, Token>
  owners: WeilMap<TokenId, Address>
  owned: WeilMap<Address, EsSet<TokenId>>
  allowances: WeilMap<string, Address>

  constructor(name: string) {
    this.name = name
    this.creator = Runtime.sender()
    this.tokens = new WeilMap<TokenId, Token>(new WeilId(1))
    this.owners = new WeilMap<TokenId, Address>(new WeilId(2))
    this.owned = new WeilMap<Address, EsSet<TokenId>>(new WeilId(3))
    this.allowances = new WeilMap<string, Address>(new WeilId(4))
  }

  isValidId(tokenId: TokenId): boolean {
    return tokenId.length > 0 && tokenId.length < 256
  }

  hasBeenMinted(tokenId: TokenId): boolean {
    const owner = this.owners.get(tokenId)
    return owner != null && owner != EMPTY_ADDRESS
  }

  balanceOf(addr: Address): u64 {
    const nfts = this.owned.get(addr)
    return nfts ? nfts.size : 0
  }

  ownerOf(tokenId: TokenId): Result<Address, StringError> {
    if (!this.isValidId(tokenId)) {
      return Result.Err<Address, StringError>(
        new StringError(`${tokenId} is not a valid token id`),
      )
    }

    const owner = this.owners.get(tokenId)

    if (owner) {
      return Result.Ok<Address, StringError>(owner)
    } else {
      return Result.Err<Address, StringError>(
        new StringError(`owner of "${tokenId}" is not identified`),
      )
    }
  }

  details(tokenId: TokenId): Result<Token, DetailsFetchError> {
    if (!this.isValidId(tokenId)) {
      return Result.Err<Token, DetailsFetchError>(
        DetailsFetchError.InvalidTokenId(tokenId),
      )
    }

    if (!this.hasBeenMinted(tokenId)) {
      return Result.Err<Token, DetailsFetchError>(
        DetailsFetchError.TokenNotMinted(tokenId),
      )
    }

    const token = this.tokens.get(tokenId)
    if (!token) {
      return Result.Err<Token, DetailsFetchError>(
        DetailsFetchError.TokenNotFound(tokenId),
      )
    }

    return Result.Ok<Token, DetailsFetchError>(token)
  }

  private doTransfer(
    tokenId: TokenId,
    fromAddr: Address,
    toAddr: Address,
  ): Result<string, StringError> {
    const ledgerResult = Ledger.transfer(tokenId, fromAddr, toAddr, 1)

    if (ledgerResult.isErr()) {
      return Result.Err<string, StringError>(
        new StringError(`"${tokenId}" could not be transferred by the Ledger`),
      )
    }

    this.owners.set(tokenId, toAddr)

    const fromTokensOrNull: EsSet<TokenId> | null = this.owned.get(fromAddr)
    let fromTokens: EsSet<TokenId> = new EsSet<TokenId>()
    if (fromTokensOrNull !== null) {
      fromTokens = fromTokensOrNull
    }

    fromTokens.delete(tokenId)
    this.owned.set(fromAddr, fromTokens)

    const toTokensOrNull: EsSet<TokenId> | null = this.owned.get(toAddr)
    let toTokens: EsSet<TokenId> = new EsSet<TokenId>()
    if (toTokensOrNull !== null) {
      toTokens = toTokensOrNull
    }

    toTokens.add(tokenId)
    this.owned.set(toAddr, toTokens)

    const key = getAllowanceKey(fromAddr, tokenId)
    this.allowances.delete(key)

    return Result.Ok<string, StringError>('null')
  }

  transfer(toAddr: Address, tokenId: TokenId): Result<string, StringError> {
    const fromAddr = Runtime.sender()

    if (!this.isValidId(tokenId)) {
      return Result.Err<string, StringError>(
        new StringError(`Invalid TokenId: ${tokenId}`),
      )
    }

    const owner = this.owners.get(tokenId)
    if (owner !== fromAddr) {
      return Result.Err<string, StringError>(
        new StringError(`Token ${tokenId} not owned by ${fromAddr}`),
      )
    }

    return this.doTransfer(tokenId, fromAddr, toAddr)
  }

  transferFrom(
    fromAddr: Address,
    toAddr: Address,
    tokenId: TokenId,
  ): Result<string, StringError> {
    const spender = Runtime.sender()

    // Validate token ID
    if (!this.isValidId(tokenId)) {
      return Result.Err<string, StringError>(
        new StringError(`Invalid TokenId: ${tokenId}`),
      )
    }

    // Check ownership of the token
    const owner = this.owners.get(tokenId)
    if (!owner || owner !== fromAddr) {
      return Result.Err<string, StringError>(
        new StringError(`Token ${tokenId} not owned by ${fromAddr}`),
      )
    }

    // Check allowances for specific or global approval
    const specificAllowanceKey = getAllowanceKey(fromAddr, tokenId)
    const globalAllowanceKey = getAllowanceKey(fromAddr, EMPTY_TOKEN_ID)
    const isAllowed =
      this.allowances.get(specificAllowanceKey) === spender ||
      this.allowances.get(globalAllowanceKey) === spender

    if (!isAllowed) {
      return Result.Err<string, StringError>(
        new StringError(
          "transfer of token `" + tokenId +"` not authorized",
        ),
      )
    }

    // Execute the transfer
    this.doTransfer(tokenId, fromAddr, toAddr)

    return Result.Ok<string, StringError>('null')
  }

  // Approve an address to transfer a specific token on behalf of the owner
  approve(spender: Address, tokenId: TokenId): Result<string, StringError> {
    const fromAddr = Runtime.sender()

    // Validate the token ID
    if (!this.isValidId(tokenId)) {
      return Result.Err<string, StringError>(
        new StringError(`Invalid token ID: ${tokenId}`),
      )
    }

    // Check if the token has an owner
    const owner = this.owners.get(tokenId)
    if (!owner) {
      return Result.Err<string, StringError>(
        new StringError(`Token ${tokenId} is missing an owner`),
      )
    }

    // Ensure that the owner is the caller
    if (owner != fromAddr) {
      return Result.Err<string, StringError>(
        new StringError(`Allowance of token ${tokenId} not authorized`),
      )
    }

    // Generate the allowance key
    const key = getAllowanceKey(fromAddr, tokenId)

    // If the spender address is empty, remove the approval; otherwise, set the approval
    if (spender == EMPTY_ADDRESS) {
      this.allowances.delete(key)
    } else {
      this.allowances.set(key, spender)
    }

    return Result.Ok<string, StringError>('null')
  }

  // Check if an address is approved for a specific token or all tokens of an owner
  getApproved(tokenId: TokenId): Array<Address> {
    const response: Array<Address> = []

    // Validate the token ID
    if (!this.isValidId(tokenId)) {
      throw new Error(`Invalid token ID: ${tokenId}`)
    }

    // Check if the token has an owner
    const owner = this.owners.get(tokenId)
    if (!owner) {
      throw new Error(`Token ${tokenId} is missing an owner`)
    }

    // Check for specific and global allowances
    const specificKey = getAllowanceKey(owner, tokenId)
    const globalKey = getAllowanceKey(owner, EMPTY_TOKEN_ID)

    const specificAllowance = this.allowances.get(specificKey)
    if (specificAllowance) {
      response.push(specificAllowance)
    }

    const globalAllowance = this.allowances.get(globalKey)
    if (globalAllowance) {
      response.push(globalAllowance)
    }

    return response
  }

  // Check if an address is approved for all tokens of a given owner
  isApprovedForAll(owner: Address, spender: Address): bool {
    const key = getAllowanceKey(owner, EMPTY_TOKEN_ID)
    const allowed = this.allowances.get(key)
    return allowed == spender
  }

  // Approve or disapprove an address for all tokens of the caller
  setApproveForAll(spender: Address, approval: bool): void {
    const fromAddr = Runtime.sender()

    const key = getAllowanceKey(fromAddr, EMPTY_TOKEN_ID)
    if (approval) {
      this.allowances.set(key, spender)
    } else {
      this.allowances.delete(key)
    }
  }

  mint(tokenId: TokenId, token: Token): Result<string, StringError> {
    const fromAddr = Runtime.sender()

    if (this.hasBeenMinted(tokenId)) {
      return Result.Err<string, StringError>(
        new StringError(`Token ${tokenId} has already been minted`),
      )
    }

    const ledgerResult = Ledger.mint(tokenId, fromAddr, 1)
    if (ledgerResult.isErr()) {
      return Result.Err<string, StringError>(
        new StringError(`${tokenId} could not be minted by the Ledger`),
      )
    }

    this.tokens.set(tokenId, token)
    this.owners.set(tokenId, fromAddr)

    let ownerTokensOrNull: EsSet<TokenId> | null = this.owned.get(fromAddr)
    let ownerTokens: EsSet<TokenId> =
      ownerTokensOrNull === null ? new EsSet<TokenId>() : ownerTokensOrNull

    ownerTokens.add(tokenId)
    this.owned.set(fromAddr, ownerTokens)

    return Result.Ok<string, StringError>('null')
  }
}
