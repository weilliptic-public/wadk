package collections

import (
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/internal/constants"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

type WeilVec[T any] struct {
	StateId WeilId `json:"state_id"`
	Len     uint   `json:"len"`
}

func NewWeilVec[T any](stateId WeilId) *WeilVec[T] {
	return &WeilVec[T]{
		StateId: stateId,
		Len:     0,
	}
}

// Implement WeilCollection

func (v *WeilVec[T]) BaseStatePath() string {
	return v.StateId.String()
}

func (v *WeilVec[T]) StateTreeKey(suffix uint) string {
	return fmt.Sprintf("%s_%d", v.BaseStatePath(), suffix)
}

// Implement WeilVec

func (v *WeilVec[T]) Push(item *T) {
	index := v.Len
	runtime.WriteCollectionEntry([]byte(v.StateTreeKey(index)), item)
	v.Len += 1
}

func (v *WeilVec[T]) Get(index uint) (*T, error) {
	if index >= v.Len {
		return nil, fmt.Errorf("index out of bounds: length of vector is `%d`, index received `%d`", v.Len, index)
	}

	item, err := runtime.ReadCollection[T]([]byte(v.StateTreeKey(index)))

	if err != nil {
		panic(constants.UNREACHABLE)
	}

	return item, nil
}

func (v *WeilVec[T]) Set(index uint, item *T) error {
	if index >= v.Len {
		return fmt.Errorf("index out of bounds: length of vector is `%d`, index received `%d`", v.Len, index)
	}

	runtime.WriteCollectionEntry([]byte(v.StateTreeKey(index)), item)

	return nil
}
