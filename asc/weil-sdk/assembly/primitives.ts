import { JSON } from 'json-as'

function isString<U>(value: U): bool {
  return value instanceof String
}

function safeToString<U>(value: U): string {
  if (value instanceof String) {
    return value // Already a string, return as is
  }

  return ''
}

@json
export class Box<T> {
  value: T

  constructor(value: T) {
    this.value = value
  }
}

@json
export class StateResultValue {
  state: string | null
  value: string

  constructor(state: string | null, value: string) {
    this.state = state
    this.value = value
  }

  static new(state: string | null, value: string): StateResultValue {
    return new StateResultValue(state, value)
  }
}

@json
export class StateArgsValue {
  state: string
  args: string

  constructor(state: string, args: string) {
    this.state = state
    this.args = args
  }

  static new(state: string, args: string): StateArgsValue {
    return new StateArgsValue(state, args)
  }
}

@json
export class WeilValue<T, U> {
  state: T | null
  ok_val: U

  constructor(state: T | null, ok_val: U) {
    this.state = state
    this.ok_val = ok_val
  }

  static newWithOkValue<U>(val: U): WeilValue<string, U> {
    return new WeilValue<string, U>(null, val)
  }

  static newWithStateAndOkValue<T, U>(state: T, val: U): WeilValue<T, U> {
    return new WeilValue<T, U>(state, val)
  }

  hasState(): boolean {
    return this.state !== null
  }

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
