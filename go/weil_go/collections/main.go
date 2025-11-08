// Package collections provides data structures for managing state in Weil contracts.
// It includes implementations of Map, Set, and Vector collections that can be
// persisted to the contract's state tree.
package collections

import (
	"fmt"
)

// WeilId represents a unique identifier for a collection in the state tree.
// It is used to distinguish different collections within a contract's state.
type WeilId struct {
	Id uint8 `json:"id"`
}

// NewWeilId creates a new WeilId with the given identifier.
func NewWeilId(id uint8) *WeilId {
	return &WeilId{id}
}

// String returns the string representation of the WeilId.
func (id *WeilId) String() string {
	return fmt.Sprintf("%d", id.Id)
}

// WeilCollections is an interface that all Weil collection types must implement.
// It provides methods for accessing the base state path and generating state tree keys.
type WeilCollections[T any] interface {
	// BaseStatePath returns the base path for this collection in the state tree.
	BaseStatePath() string
	// StateTreeKey generates a state tree key for the given suffix value.
	StateTreeKey(suffix T) []byte
}
