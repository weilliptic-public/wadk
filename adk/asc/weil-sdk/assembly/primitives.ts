import { JSON } from 'json-as'

/**
 * Checks if a value is a string instance.
 * 
 * @template U - The type of value to check
 * @param value - The value to check
 * @returns True if the value is a string instance
 */
function isString<U>(value: U): bool {
  return value instanceof String
}

/**
 * Safely converts a value to a string, handling string instances.
 * 
 * @template U - The type of value to convert
 * @param value - The value to convert
 * @returns The string representation, or empty string if not a string
 */
function safeToString<U>(value: U): string {
  if (value instanceof String) {
    return value // Already a string, return as is
  }

  return ''
}

/**
 * A generic box type that wraps a value.
 * Useful for wrapping primitive types in collections or function returns.
 * 
 * @template T - The type of value to wrap
 */
@json
export class Box<T> {
  /** The wrapped value */
  value: T

  /**
   * Creates a new Box instance.
   * 
   * @param value - The value to wrap
   */
  constructor(value: T) {
    this.value = value
  }
}

/**
 * Represents a state and result value pair for contract responses.
 */
@json
export class StateResultValue {
  /** The serialized state, or null if no state update */
  state: string | null
  /** The serialized result value */
  value: string

  /**
   * Creates a new StateResultValue instance.
   * 
   * @param state - The serialized state, or null
   * @param value - The serialized result value
   */
  constructor(state: string | null, value: string) {
    this.state = state
    this.value = value
  }

  /**
   * Creates a new StateResultValue instance.
   * 
   * @param state - The serialized state, or null
   * @param value - The serialized result value
   * @returns A new StateResultValue instance
   */
  static new(state: string | null, value: string): StateResultValue {
    return new StateResultValue(state, value)
  }
}

/**
 * Represents a state and arguments value pair for contract method calls.
 */
@json
export class StateArgsValue {
  /** The serialized state */
  state: string
  /** The serialized method arguments */
  args: string

  /**
   * Creates a new StateArgsValue instance.
   * 
   * @param state - The serialized state
   * @param args - The serialized method arguments
   */
  constructor(state: string, args: string) {
    this.state = state
    this.args = args
  }

  /**
   * Creates a new StateArgsValue instance.
   * 
   * @param state - The serialized state
   * @param args - The serialized method arguments
   * @returns A new StateArgsValue instance
   */
  static new(state: string, args: string): StateArgsValue {
    return new StateArgsValue(state, args)
  }
}

/**
 * A generic value type that can hold both state and a result value.
 * Used for contract method returns that may update state.
 * 
 * @template T - The type of the state
 * @template U - The type of the result value
 */
@json
export class WeilValue<T, U> {
  /** The state value, or null if no state update */
  state: T | null
  /** The result value */
  ok_val: U

  /**
   * Creates a new WeilValue instance.
   * 
   * @param state - The state value, or null
   * @param ok_val - The result value
   */
  constructor(state: T | null, ok_val: U) {
    this.state = state
    this.ok_val = ok_val
  }

  /**
   * Creates a WeilValue with only an OK value (no state update).
   * 
   * @template U - The type of the result value
   * @param val - The result value
   * @returns A WeilValue with null state and the provided value
   */
  static newWithOkValue<U>(val: U): WeilValue<string, U> {
    return new WeilValue<string, U>(null, val)
  }

  /**
   * Creates a WeilValue with both state and an OK value.
   * 
   * @template T - The type of the state
   * @template U - The type of the result value
   * @param state - The state value
   * @param val - The result value
   * @returns A WeilValue with the provided state and value
   */
  static newWithStateAndOkValue<T, U>(state: T, val: U): WeilValue<T, U> {
    return new WeilValue<T, U>(state, val)
  }

  /**
   * Checks if this WeilValue has a state update.
   * 
   * @returns True if state is not null
   */
  hasState(): boolean {
    return this.state !== null
  }

  /**
   * Converts this WeilValue to a StateResultValue for serialization.
   * 
   * @returns A StateResultValue with serialized state and value
   */
  raw(): StateResultValue {
    const stateJson = this.hasState() ? JSON.stringify(this.state) : null
  
    if (isString(this.ok_val) && safeToString(this.ok_val) === "null") {
      return StateResultValue.new(stateJson, "null")
    }
    
    const okVal = this.ok_val
    let okValueString = JSON.stringify(this.ok_val)
  
    if (isString(okVal)) {
      okValueString = safeToString(okVal)
    }
  
    return StateResultValue.new(stateJson, okValueString)
  }
}
