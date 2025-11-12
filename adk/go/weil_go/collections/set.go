package collections

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

type __unit__ struct{}

// WeilSet is a set collection that stores unique values of comparable type.
// It is implemented internally using a WeilMap with unit values.
type WeilSet[V comparable] struct {
	InnerMap WeilMap[V, __unit__] `json:"map"`
}

// NewWeilSet creates a new WeilSet with the given state identifier.
func NewWeilSet[V comparable](stateId WeilId) *WeilSet[V] {
	return &WeilSet[V]{
		InnerMap: WeilMap[V, __unit__]{stateId},
	}
}

// BaseStatePath returns the base path for this set in the state tree.
func (m *WeilSet[V]) BaseStatePath() string {
	return m.InnerMap.BaseStatePath()
}

// StateTreeKey generates a state tree key for the given value suffix.
func (m *WeilSet[V]) StateTreeKey(suffix *V) string {
	serializedSuffix, _ := json.Marshal(suffix)
	return fmt.Sprintf("%s_%s", m.BaseStatePath(), string(serializedSuffix))
}

// Insert adds a value to the set.
// If the value already exists, this operation has no effect.
func (s *WeilSet[V]) Insert(val *V) {
	runtime.WriteCollectionEntry[V]([]byte(s.StateTreeKey(val)), nil)
}

// Contains checks whether the given value exists in the set.
// Returns true if the value is present, false otherwise.
func (s *WeilSet[V]) Contains(val *V) bool {
	_, err := runtime.ReadCollection[V]([]byte(s.StateTreeKey(val)))
	if err != nil {
		switch err.(type) {
		case *errors.KeyNotFoundInCollectionError:
			return false
		default:
			panic(fmt.Sprintf("error other than `KeyNotFoundInCollection` received while `ReadCollection`: %v", err.Error()))
		}
	}

	return true
}

// Remove removes the given value from the set.
// Returns true if the value was removed, false if it was not in the set.
func (s *WeilSet[V]) Remove(value *V) (bool, error) {
	_, err := runtime.DeleteCollectionEntry[V]([]byte(s.StateTreeKey(value)))
	if err != nil {
		switch err.(type) {
		case *errors.KeyNotFoundInCollectionError:
			return false, nil
		default:
			panic(fmt.Sprintf("error other than `KeyNotFoundInCollection` received while `ReadCollection`: %v", err.Error()))
		}
	}

	return true, nil
}
