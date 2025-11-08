import { JSON } from 'json-as'

export class JsonSerializable {
  toJSON(): string {
    return ''
  }
}

export function getJsonSerializable<T>(value: T): JsonSerializable | null {
  if (value instanceof JsonSerializable) {
    return value as JsonSerializable
  }

  return null
}

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
