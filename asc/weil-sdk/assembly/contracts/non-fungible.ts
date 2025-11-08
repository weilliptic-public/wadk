import { WeilMap } from '../collections/map'
import { WeilId } from '../collections/weil-id'
import { Result } from '../result'
import { DetailsFetchError } from './DetailsFetchError'
import { Runtime } from '../runtime'
import { StringError } from '../error'
import { Ledger } from '../ledger'

/**
 * A simple set implementation using an array.
 * Used internally for managing token ownership sets.
 * 
 * @template K - The type of values in the set
 */
@json
class EsSet<K> {
  /** The internal array storing set values */
  inner: K[]

  /**
   * Gets the size of the set.
   * 
   * @returns The number of elements in the set
   */
  get size(): i32 {
    return this.inner.length
  }

  /**
   * Checks if a value exists in the set.
   * 
   * @param value - The value to check
   * @returns True if the value exists, false otherwise
   */
  has(value: K): bool {
    return this.inner.includes(value)
  }

  /**
   * Adds a value to the set if it doesn't already exist.
   * 
   * @param value - The value to add
   * @returns This set instance for chaining
   */
  add(value: K): this {
    if (!this.has(value)) {
      this.inner.push(value)
    }

    return this
  }

  /**
   * Removes a value from the set.
   * 
   * @param value - The value to remove
   * @returns True if the value was removed, false if it didn't exist
   */
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

  /**
   * Removes all values from the set.
   */
  clear(): void {
    this.inner.length = 0
  }
  
  /**
   * Gets all values in the set as an array.
   * 
   * @returns An array containing all set values
   */
  values(): K[] {
    return [...this.inner]
  }

  /**
   * Converts the set to a string representation.
   * 
   * @returns A string representation of the set
   */
  toString(): string {
    return this.inner.toString()
  }

  /**
   * Creates a new EsSet instance.
   */
  constructor() {
    this.inner = []
  }
}

/**
 * Represents a non-fungible token with metadata.
 */
@json
export class Token {
  /** The title of the token */
  title: string
  /** The name of the token */
  name: string
  /** The description of the token */
  description: string
  /** Additional payload data for the token */
  payload: string

  /**
   * Creates a new Token instance.
   * 
   * @param title - The title of the token
   * @param name - The name of the token
   * @param description - The description of the token
   * @param payload - Additional payload data
   */
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

/** Type alias for token ID */
export type TokenId = string
/** Empty token ID constant */
const EMPTY_TOKEN_ID: string = ''
/** Type alias for address */
type Address = string
/** Empty address constant */
const EMPTY_ADDRESS: string = ''

/**
 * Arguments for the balanceOf method.
 */
@json
export class BalanceOfArgs {
  /** The address to query */
  addr!: string
}

/**
 * Arguments for methods that require a token ID.
 */
@json
export class TokenIdArgs {
  /** The token ID */
  token_id!: string | null
}

/**
 * Arguments for the approve method.
 */
@json
export class ApproveArgs {
  /** The spender address to approve */
  spender!: string | null
  /** The token ID to approve */
  token_id!: string | null
}

/**
 * Arguments for the setApproveForAll method.
 */
@json
export class SetApproveForAllArgs {
  /** The spender address */
  spender!: string | null
  /** Whether to approve (true) or revoke (false) */
  approval!: boolean
}

/**
 * Arguments for the isApprovedForAll method.
 */
@json
export class IsApprovedForAllArgs {
  /** The owner address */
  owner!: string | null
  /** The spender address */
  spender!: string | null
}

/**
 * Arguments for the transfer method.
 */
@json
export class TransferArgs {
  /** The address to transfer to */
  to_addr!: string | null
  /** The token ID to transfer */
  token_id!: string | null
}

/**
 * Arguments for the transferFrom method.
 */
@json
export class TransferFromArgs {
  /** The address to transfer from */
  from_addr!: string | null
  /** The address to transfer to */
  to_addr!: string | null
  /** The token ID to transfer */
  token_id!: string | null
}

/**
 * Arguments for the mint method.
 */
@json
export class MintArgs {
  /** The token ID to mint */
  token_id!: string | null
  /** The title of the token */
  title!: string | null
  /** The name of the token */
  name!: string | null
  /** The description of the token */
  description!: string | null
  /** The payload data for the token */
  payload!: string | null
}

/**
 * Generates a unique key for an allowance mapping.
 * 
 * @param fromAddr - The owner address
 * @param tokenId - The token ID (use EMPTY_TOKEN_ID for global approvals)
 * @returns A unique key string
 */
const getAllowanceKey = (fromAddr: string, tokenId: string): string =>
  [fromAddr, tokenId].join('$')

/**
 * NonFungibleToken class implementing ERC-721-like functionality.
 * Manages unique tokens with ownership, transfers, approvals, and minting.
 */
@json
export class NonFungibleToken {
  /** The name of the NFT collection */
  name: string
  /** The address of the collection creator */
  creator: Address
  /** Map of token IDs to token metadata */
  tokens: WeilMap<TokenId, Token>
  /** Map of token IDs to owner addresses */
  owners: WeilMap<TokenId, Address>
  /** Map of addresses to sets of owned token IDs */
  owned: WeilMap<Address, EsSet<TokenId>>
  /** Map of allowance keys to approved spender addresses */
  allowances: WeilMap<string, Address>

  /**
   * Creates a new NonFungibleToken instance.
   * 
   * @param name - The name of the NFT collection
   */
  constructor(name: string) {
    this.name = name
    this.creator = Runtime.sender()
    this.tokens = new WeilMap<TokenId, Token>(new WeilId(1))
    this.owners = new WeilMap<TokenId, Address>(new WeilId(2))
    this.owned = new WeilMap<Address, EsSet<TokenId>>(new WeilId(3))
    this.allowances = new WeilMap<string, Address>(new WeilId(4))
  }

  /**
   * Validates whether a token ID is valid.
   * 
   * @param tokenId - The token ID to validate
   * @returns True if the token ID is valid (length between 1 and 255)
   */
  isValidId(tokenId: TokenId): boolean {
    return tokenId.length > 0 && tokenId.length < 256
  }

  /**
   * Checks if a token has been minted.
   * 
   * @param tokenId - The token ID to check
   * @returns True if the token has been minted
   */
  hasBeenMinted(tokenId: TokenId): boolean {
    const owner = this.owners.get(tokenId)
    return owner != null && owner != EMPTY_ADDRESS
  }

  /**
   * Gets the balance (number of tokens owned) for a given address.
   * 
   * @param addr - The address to query
   * @returns The number of tokens owned by the address
   */
  balanceOf(addr: Address): u64 {
    const nfts = this.owned.get(addr)
    return nfts ? nfts.size : 0
  }

  /**
   * Gets the owner of a specific token.
   * 
   * @param tokenId - The token ID to query
   * @returns A Result containing the owner address or an error
   */
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

  /**
   * Gets the details (metadata) of a specific token.
   * 
   * @param tokenId - The token ID to query
   * @returns A Result containing the token details or an error
   */
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

  /**
   * Internal method to perform a token transfer.
   * Updates ownership, owned sets, and clears allowances.
   * 
   * @param tokenId - The token ID to transfer
   * @param fromAddr - The address to transfer from
   * @param toAddr - The address to transfer to
   * @returns A Result indicating success or failure
   */
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

  /**
   * Transfers a token from the sender to another address.
   * 
   * @param toAddr - The address to transfer to
   * @param tokenId - The token ID to transfer
   * @returns A Result indicating success or failure
   */
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

  /**
   * Transfers a token from one address to another on behalf of the sender.
   * Requires the sender to have approval from the fromAddr.
   * 
   * @param fromAddr - The address to transfer from
   * @param toAddr - The address to transfer to
   * @param tokenId - The token ID to transfer
   * @returns A Result indicating success or failure
   */
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

  /**
   * Approves an address to transfer a specific token on behalf of the owner.
   * 
   * @param spender - The address to approve as a spender
   * @param tokenId - The token ID to approve
   * @returns A Result indicating success or failure
   */
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

  /**
   * Gets the approved addresses for a specific token.
   * Returns both specific approvals and global approvals for the token owner.
   * 
   * @param tokenId - The token ID to query
   * @returns An array of approved addresses
   * @throws {Error} If the token ID is invalid or the token has no owner
   */
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

  /**
   * Checks if an address is approved for all tokens of a given owner.
   * 
   * @param owner - The owner address
   * @param spender - The spender address to check
   * @returns True if the spender is approved for all tokens of the owner
   */
  isApprovedForAll(owner: Address, spender: Address): bool {
    const key = getAllowanceKey(owner, EMPTY_TOKEN_ID)
    const allowed = this.allowances.get(key)
    return allowed == spender
  }

  /**
   * Approves or revokes approval for an address to transfer all tokens of the caller.
   * 
   * @param spender - The address to approve or revoke
   * @param approval - True to approve, false to revoke
   */
  setApproveForAll(spender: Address, approval: bool): void {
    const fromAddr = Runtime.sender()

    const key = getAllowanceKey(fromAddr, EMPTY_TOKEN_ID)
    if (approval) {
      this.allowances.set(key, spender)
    } else {
      this.allowances.delete(key)
    }
  }

  /**
   * Mints a new token with the given ID and metadata.
   * 
   * @param tokenId - The token ID to mint
   * @param token - The token metadata
   * @returns A Result indicating success or failure
   */
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
