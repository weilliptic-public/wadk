// Package types provides generic type utilities for Weil contracts, including
// Option, Result, and Tuple types that are commonly used in functional programming.
package types

import (
	"encoding/json"
)

// Option represents an optional value that may or may not be present.
// It is similar to Rust's Option type or Haskell's Maybe type.
type Option[T any] struct {
	some *T
	none bool
}

// NewSomeOption creates a new Option with a value.
// Panics if val is nil; use NewNoneOption to represent the absence of a value.
func NewSomeOption[T any](val *T) *Option[T] {
	if val == nil {
		panic("nil value not allowed, use `NewNoneOption` to depict the absense of value")
	}

	return &Option[T]{
		some: val,
		none: false,
	}
}

// NewNoneOption creates a new Option representing the absence of a value.
func NewNoneOption[T any]() *Option[T] {
	return &Option[T]{
		some: nil,
		none: true,
	}
}

// TrySomeOption returns the contained value if present, or nil if the Option is None.
func (obj *Option[T]) TrySomeOption() *T {
	return obj.some
}

// IsSomeResult returns true if the Option contains a value.
func (obj *Option[T]) IsSomeResult() bool {
	return !obj.none
}

// IsNoneResult returns true if the Option does not contain a value.
func (obj *Option[T]) IsNoneResult() bool {
	return obj.none
}

// MarshalJSON implements the json.Marshaler interface for Option.
// None values are marshaled as null, Some values are marshaled as their contained value.
func (obj Option[T]) MarshalJSON() ([]byte, error) {
	if obj.IsNoneResult() {
		return []byte("null"), nil
	} else {
		return json.Marshal(&obj.some)
	}
}

// UnmarshalJSON implements the json.Unmarshaler interface for Option.
// null values are unmarshaled as None, other values are unmarshaled as Some.
func (obj *Option[T]) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		obj.none = true
		obj.some = nil

		return nil
	}

	var tmp T
	err := json.Unmarshal(data, &tmp)

	if err != nil {
		return err
	}

	obj.some = &tmp
	obj.none = false

	return nil
}
