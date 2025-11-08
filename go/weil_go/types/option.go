package types

import (
	"encoding/json"
)

type Option[T any] struct {
	some *T
	none bool
}

func NewSomeOption[T any](val *T) *Option[T] {
	if val == nil {
		panic("nil value not allowed, use `NewNoneOption` to depict the absense of value")
	}

	return &Option[T]{
		some: val,
		none: false,
	}
}

func NewNoneOption[T any]() *Option[T] {
	return &Option[T]{
		some: nil,
		none: true,
	}
}

func (obj *Option[T]) TrySomeOption() *T {
	return obj.some
}

func (obj *Option[T]) IsSomeResult() bool {
	return !obj.none
}

func (obj *Option[T]) IsNoneResult() bool {
	return obj.none
}

func (obj Option[T]) MarshalJSON() ([]byte, error) {
	if obj.IsNoneResult() {
		return []byte("null"), nil
	} else {
		return json.Marshal(&obj.some)
	}
}

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
