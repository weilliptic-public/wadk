package types

import (
	"encoding/json"
	"fmt"
)

// Result represents a value that can be either a success (Ok) or an error (Err).
// It is similar to Rust's Result type and provides a way to handle errors without exceptions.
type Result[T any, E any] struct {
	val *T
	err *E
}

// NewOkResult creates a new Result representing a successful value.
func NewOkResult[T any, E any](val *T) *Result[T, E] {
	return &Result[T, E]{
		val: val,
		err: nil,
	}
}

// NewErrResult creates a new Result representing an error.
func NewErrResult[T any, E any](err *E) *Result[T, E] {
	return &Result[T, E]{
		val: nil,
		err: err,
	}
}

// TryOkResult returns the contained value if the Result is Ok, or nil if it is Err.
func (obj *Result[T, E]) TryOkResult() *T {
	return obj.val
}

// TryErrResult returns the contained error if the Result is Err, or nil if it is Ok.
func (obj *Result[T, E]) TryErrResult() *E {
	return obj.err
}

// IsOkResult returns true if the Result contains a successful value.
func (obj *Result[T, E]) IsOkResult() bool {
	return obj.err == nil
}

// IsErrResult returns true if the Result contains an error.
func (obj *Result[T, E]) IsErrResult() bool {
	return obj.err != nil
}

// MarshalJSON implements the json.Marshaler interface for Result.
// Results are marshaled as a map with either an "Ok" or "Err" key.
func (obj Result[T, E]) MarshalJSON() ([]byte, error) {
	m := make(map[string]interface{}, 2)

	if obj.err != nil {
		m["Err"] = obj.err
	} else {
		m["Ok"] = obj.val
	}

	return json.Marshal(&m)
}

// UnmarshalJSON implements the json.Unmarshaler interface for Result.
// Expects a map with either an "Ok" or "Err" key.
func (obj *Result[T, E]) UnmarshalJSON(data []byte) error {
	var tmp map[string]interface{}
	err := json.Unmarshal(data, &tmp)

	if err != nil {
		return err
	}

	if len(tmp) != 1 {
		return fmt.Errorf("enum-type unmarshalling expects exactly one key at 0 level (variant name)")
	}

	var criticalEnumVariant string

	for _, key := range []string{"Ok", "Err"} {
		_, ok := tmp[key]

		if ok {
			criticalEnumVariant = key
			break
		}
	}

	if criticalEnumVariant == "" {
		return fmt.Errorf(`enum-type unmarshalling expects key from variant names: ["Ok", "Err"]`)
	}

	entry, _ := json.Marshal(tmp[criticalEnumVariant])

	switch criticalEnumVariant {
	case "Ok":
		var val T
		valP := &val
		err = json.Unmarshal(entry, &valP)

		if err != nil {
			return err
		}

		obj.val = valP
		obj.err = nil

	case "Err":
		var val E
		err = json.Unmarshal(entry, &val)

		if err != nil {
			return err
		}

		obj.val = nil
		obj.err = &val

	}

	return nil
}
