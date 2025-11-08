import { JSON } from 'json-as'

/**
 * Base class for types that can be serialized to JSON.
 * Classes that extend this should implement the toJSON() method.
 */
export class JsonSerializable {
  /**
   * Serializes the object to a JSON string.
   * 
   * @returns A JSON string representation of the object
   */
  toJSON(): string {
    return ''
  }
}

/**
 * Checks if a value implements JsonSerializable and returns it if so.
 * 
 * @template T - The type of value to check
 * @param value - The value to check
 * @returns The value as JsonSerializable if it implements it, null otherwise
 */
export function getJsonSerializable<T>(value: T): JsonSerializable | null {
  if (value instanceof JsonSerializable) {
    return value as JsonSerializable
  }

  return null
}

/**
 * Serializes a value to JSON, using toJSON() if it implements JsonSerializable,
 * otherwise using JSON.stringify().
 * 
 * @template T - The type of value to serialize
 * @param value - The value to serialize
 * @returns A JSON string representation of the value
 */
export function serializeToJson<T>(value: T): string {
  let serializedResult = ''
  const serializable = getJsonSerializable<T>(value)
  if (serializable) {
    serializedResult = serializable.toJSON()
  } else {
    serializedResult = JSON.stringify<T>(value)
  }

  return serializedResult
}
