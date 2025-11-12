package types

import (
	"encoding/json"
	"fmt"
)

// Tuple2 represents a tuple of two values with generic types.
type Tuple2[T any, U any] struct {
	F0 T
	F1 U
}

// NewTuple2 creates a new Tuple2 with the given values.
func NewTuple2[T any, U any](f0 T, f1 U) *Tuple2[T, U] {
	return &Tuple2[T, U]{
		F0: f0,
		F1: f1,
	}
}

// MarshalJSON implements the json.Marshaler interface for Tuple2.
// Tuples are marshaled as JSON arrays.
func (obj Tuple2[T, U]) MarshalJSON() ([]byte, error) {
	var v []interface{}

	v = append(v, obj.F0)
	v = append(v, obj.F1)

	return json.Marshal(v)
}

// UnmarshalJSON implements the json.Unmarshaler interface for Tuple2.
// Expects a JSON array with exactly 2 elements.
func (obj *Tuple2[T, U]) UnmarshalJSON(data []byte) error {
	var v []interface{}

	err := json.Unmarshal(data, &v)

	if err != nil {
		return err
	}

	if len(v) != 2 {
		return fmt.Errorf("`types.Tuple2` unmarshalling expected 2 entries, instead got %d", len(v))
	}

	var f0 T
	var f1 U

	serializedF0, _ := json.Marshal(&v[0])
	serializedF1, _ := json.Marshal(&v[1])

	json.Unmarshal(serializedF0, &f0)
	json.Unmarshal(serializedF1, &f1)

	obj.F0 = f0
	obj.F1 = f1

	return nil
}

// Tuple3 represents a tuple of three values with generic types.
type Tuple3[T any, U any, V any] struct {
	F0 T
	F1 U
	F2 V
}

// NewTuple3 creates a new Tuple3 with the given values.
func NewTuple3[T any, U any, V any](f0 T, f1 U, f2 V) *Tuple3[T, U, V] {
	return &Tuple3[T, U, V]{
		F0: f0,
		F1: f1,
		F2: f2,
	}
}

// MarshalJSON implements the json.Marshaler interface for Tuple3.
func (obj Tuple3[T, U, V]) MarshalJSON() ([]byte, error) {
	var v []interface{}

	v = append(v, obj.F0)
	v = append(v, obj.F1)
	v = append(v, obj.F2)

	return json.Marshal(v)
}

// UnmarshalJSON implements the json.Unmarshaler interface for Tuple3.
// Expects a JSON array with exactly 3 elements.
func (obj *Tuple3[T, U, V]) UnmarshalJSON(data []byte) error {
	var v []interface{}

	err := json.Unmarshal(data, &v)

	if err != nil {
		return err
	}

	if len(v) != 3 {
		return fmt.Errorf("`types.Tuple3` unmarshalling expected 3 entries, instead got %d", len(v))
	}

	var f0 T
	var f1 U
	var f2 V

	serializedF0, _ := json.Marshal(&v[0])
	serializedF1, _ := json.Marshal(&v[1])
	serializedF2, _ := json.Marshal(&v[2])

	json.Unmarshal(serializedF0, &f0)
	json.Unmarshal(serializedF1, &f1)
	json.Unmarshal(serializedF2, &f2)

	obj.F0 = f0
	obj.F1 = f1
	obj.F2 = f2

	return nil
}

// Tuple4 represents a tuple of four values with generic types.
type Tuple4[T any, U any, V any, W any] struct {
	F0 T
	F1 U
	F2 V
	F3 W
}

// NewTuple4 creates a new Tuple4 with the given values.
func NewTuple4[T any, U any, V any, W any](f0 T, f1 U, f2 V, f3 W) *Tuple4[T, U, V, W] {
	return &Tuple4[T, U, V, W]{
		F0: f0,
		F1: f1,
		F2: f2,
		F3: f3,
	}
}

// MarshalJSON implements the json.Marshaler interface for Tuple4.
func (obj Tuple4[T, U, V, W]) MarshalJSON() ([]byte, error) {
	var v []interface{}

	v = append(v, obj.F0)
	v = append(v, obj.F1)
	v = append(v, obj.F2)
	v = append(v, obj.F3)

	return json.Marshal(v)
}

// UnmarshalJSON implements the json.Unmarshaler interface for Tuple4.
// Expects a JSON array with exactly 4 elements.
func (obj *Tuple4[T, U, V, W]) UnmarshalJSON(data []byte) error {
	var v []interface{}

	err := json.Unmarshal(data, &v)

	if err != nil {
		return err
	}

	if len(v) != 4 {
		return fmt.Errorf("`types.Tuple4` unmarshalling expected 4 entries, instead got %d", len(v))
	}

	var f0 T
	var f1 U
	var f2 V
	var f3 W

	serializedF0, _ := json.Marshal(&v[0])
	serializedF1, _ := json.Marshal(&v[1])
	serializedF2, _ := json.Marshal(&v[2])
	serializedF3, _ := json.Marshal(&v[3])

	json.Unmarshal(serializedF0, &f0)
	json.Unmarshal(serializedF1, &f1)
	json.Unmarshal(serializedF2, &f2)
	json.Unmarshal(serializedF3, &f3)

	obj.F0 = f0
	obj.F1 = f1
	obj.F2 = f2
	obj.F3 = f3

	return nil
}

// Tuple5 represents a tuple of five values with generic types.
type Tuple5[T any, U any, V any, W any, X any] struct {
	F0 T
	F1 U
	F2 V
	F3 W
	F4 X
}

// NewTuple5 creates a new Tuple5 with the given values.
func NewTuple5[T any, U any, V any, W any, X any](f0 T, f1 U, f2 V, f3 W, f4 X) *Tuple5[T, U, V, W, X] {
	return &Tuple5[T, U, V, W, X]{
		F0: f0,
		F1: f1,
		F2: f2,
		F3: f3,
		F4: f4,
	}
}

// MarshalJSON implements the json.Marshaler interface for Tuple5.
func (obj Tuple5[T, U, V, W, X]) MarshalJSON() ([]byte, error) {
	var v []interface{}

	v = append(v, obj.F0)
	v = append(v, obj.F1)
	v = append(v, obj.F2)
	v = append(v, obj.F3)
	v = append(v, obj.F4)

	return json.Marshal(v)
}

// UnmarshalJSON implements the json.Unmarshaler interface for Tuple5.
// Expects a JSON array with exactly 5 elements.
func (obj *Tuple5[T, U, V, W, X]) UnmarshalJSON(data []byte) error {
	var v []interface{}

	err := json.Unmarshal(data, &v)

	if err != nil {
		return err
	}

	if len(v) != 5 {
		return fmt.Errorf("`types.Tuple5` unmarshalling expected 5 entries, instead got %d", len(v))
	}

	var f0 T
	var f1 U
	var f2 V
	var f3 W
	var f4 X

	serializedF0, _ := json.Marshal(&v[0])
	serializedF1, _ := json.Marshal(&v[1])
	serializedF2, _ := json.Marshal(&v[2])
	serializedF3, _ := json.Marshal(&v[3])
	serializedF4, _ := json.Marshal(&v[4])

	json.Unmarshal(serializedF0, &f0)
	json.Unmarshal(serializedF1, &f1)
	json.Unmarshal(serializedF2, &f2)
	json.Unmarshal(serializedF3, &f3)
	json.Unmarshal(serializedF4, &f4)

	obj.F0 = f0
	obj.F1 = f1
	obj.F2 = f2
	obj.F3 = f3
	obj.F4 = f4

	return nil
}
