package collections

import (
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal/constants"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

// WeilVec is a vector (dynamic array) collection that can be persisted to the contract's state tree.
// It supports indexed access and dynamic growth.
type WeilVec[T any] struct {
	StateId WeilId `json:"state_id"`
	Len     uint   `json:"len"`
}

// NewWeilVec creates a new empty WeilVec with the given state identifier.
func NewWeilVec[T any](stateId WeilId) *WeilVec[T] {
	return &WeilVec[T]{
		StateId: stateId,
		Len:     0,
	}
}

// BaseStatePath returns the base path for this vector in the state tree.
func (v *WeilVec[T]) BaseStatePath() string {
	return v.StateId.String()
}

// StateTreeKey generates a state tree key for the given index suffix.
func (v *WeilVec[T]) StateTreeKey(suffix uint) string {
	return fmt.Sprintf("%s_%d", v.BaseStatePath(), suffix)
}

// Push appends an item to the end of the vector.
// The vector's length is automatically incremented.
func (v *WeilVec[T]) Push(item *T) {
	index := v.Len
	runtime.WriteCollectionEntry([]byte(v.StateTreeKey(index)), item)
	v.Len += 1
}

// Get retrieves the item at the given index.
// Returns an error if the index is out of bounds.
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

// Set updates the item at the given index with the provided value.
// Returns an error if the index is out of bounds.
func (v *WeilVec[T]) Set(index uint, item *T) error {
	if index >= v.Len {
		return fmt.Errorf("index out of bounds: length of vector is `%d`, index received `%d`", v.Len, index)
	}

	runtime.WriteCollectionEntry([]byte(v.StateTreeKey(index)), item)

	return nil
}
