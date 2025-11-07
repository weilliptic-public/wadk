import { JSON } from 'json-as'

/**
 * A Result type that represents either a success (Ok) or failure (Err) value.
 * Similar to Rust's Result type, used for error handling without exceptions.
 * 
 * @template T - The type of the success value
 * @template E - The type of the error value
 */
@json
export class Result<T, E> {
  /** Whether this result is Ok (true) or Err (false) */
  private _isOk: bool
  /** The success value, or null if this is an error */
  private _value: T | null
  /** The error value, or null if this is a success */
  private _error: E | null

  /**
   * Private constructor. Use Result.Ok() or Result.Err() to create instances.
   * 
   * @param isOk - Whether this result is Ok
   * @param value - The success value, or null
   * @param error - The error value, or null
   */
  private constructor(isOk: bool, value: T | null, error: E | null) {
    this._isOk = isOk
    this._value = value
    this._error = error
  }

  /**
   * Creates a successful Result with a value.
   * 
   * @template T - The type of the success value
   * @template E - The type of the error value
   * @param val - The success value
   * @returns A Result containing the success value
   */
  static Ok<T, E>(val: T): Result<T, E> {
    return new Result<T, E>(true, val, null)
  }

  /**
   * Creates an error Result with an error value.
   * 
   * @template T - The type of the success value
   * @template E - The type of the error value (defaults to string)
   * @param err - The error value
   * @returns A Result containing the error value
   */
  static Err<T, E = string>(err: E): Result<T, E> {
    return new Result<T, E>(false, null, err)
  }

  /**
   * Checks if this Result is Ok.
   * 
   * @returns True if this Result is Ok, false if it's an error
   */
  isOk(): bool {
    return this._isOk
  }

  /**
   * Checks if this Result is an error.
   * 
   * @returns True if this Result is an error, false if it's Ok
   */
  isErr(): bool {
    return !this._isOk
  }

  /**
   * Unwraps the success value. Throws an error if this Result is Err.
   * 
   * NOTE: This function can panic, so only use it inside conditional blocks
   * that eliminate the panicking condition (e.g., after checking isOk()).
   * 
   * @returns The success value
   * @throws {Error} If this Result is Err
   */
  tryValue(): T {
    if (this._value != null) {
      return this._value!
    }

    throw new Error(
      'panic occured!... result cannot be unwrapped to `Ok` value',
    )
  }

  /**
   * Unwraps the error value. Throws an error if this Result is Ok.
   * 
   * NOTE: This function can panic, so only use it inside conditional blocks
   * that eliminate the panicking condition (e.g., after checking isErr()).
   * 
   * @returns The error value
   * @throws {Error} If this Result is Ok
   */
  tryError(): E {
    if (this._error != null) {
      return this._error!
    }

    throw new Error(
      'panic occured!... result cannot be unwrapped to `Err` value',
    )
  }

  /**
   * Serializes this Result to a JSON string.
   * 
   * @returns A JSON string representation of the Result
   */
  toJSON(): string {
    if (this._isOk && this._value) {
      const result: Map<string, T> = new Map<string, T>()
      result.set('Ok', this._value)

      return JSON.stringify<Map<string, T>>(result)
    } else if (!this._isOk && this._error) {
      const result: Map<string, E> = new Map<string, E>()
      result.set('Err', this._error)

      return JSON.stringify<Map<string, E>>(result)
    }

    return '{}'
  }
}
