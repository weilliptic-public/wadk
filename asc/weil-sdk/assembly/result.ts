import { JSON } from 'json-as'

@json
export class Result<T, E> {
  private _isOk: bool
  private _value: T | null
  private _error: E | null

  private constructor(isOk: bool, value: T | null, error: E | null) {
    this._isOk = isOk
    this._value = value
    this._error = error
  }

  static Ok<T, E>(val: T): Result<T, E> {
    return new Result<T, E>(true, val, null)
  }

  static Err<T, E = string>(err: E): Result<T, E> {
    return new Result<T, E>(false, null, err)
  }

  isOk(): bool {
    return this._isOk
  }

  isErr(): bool {
    return !this._isOk
  }

  // NOTE: below functions can panic so only use them inside
  // conditional blocks which eliminates the panicking
  // condition
  tryValue(): T {
    if (this._value != null) {
      return this._value!
    }

    throw new Error(
      'panic occured!... result cannot be unwrapped to `Ok` value',
    )
  }

  tryError(): E {
    if (this._error != null) {
      return this._error!
    }

    throw new Error(
      'panic occured!... result cannot be unwrapped to `Err` value',
    )
  }

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
