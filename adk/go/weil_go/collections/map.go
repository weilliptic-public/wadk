package collections

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

// WeilMap is a key-value map collection that can be persisted to the contract's state tree.
// It supports generic key types (must be comparable) and value types.
type WeilMap[K comparable, V any] struct {
	StateId WeilId `json:"state_id"`
}

// NewWeilMap creates a new WeilMap with the given state identifier.
func NewWeilMap[K comparable, V any](stateId WeilId) *WeilMap[K, V] {
	return &WeilMap[K, V]{
		StateId: stateId,
	}
}

// BaseStatePath returns the base path for this map in the state tree.
func (m *WeilMap[K, V]) BaseStatePath() string {
	return m.StateId.String()
}

// StateTreeKey generates a state tree key for the given key suffix.
func (m *WeilMap[K, V]) StateTreeKey(suffix *K) string {
	serializedSuffix, _ := json.Marshal(suffix)
	return fmt.Sprintf("%s_%s", m.BaseStatePath(), string(serializedSuffix))
}

// Insert adds or updates a key-value pair in the map.
// If the key already exists, the value will be overwritten.
func (m *WeilMap[K, V]) Insert(key *K, val *V) {
	runtime.WriteCollectionEntry[V]([]byte(m.StateTreeKey(key)), val)
}

// Get retrieves the value associated with the given key.
// Returns an error if the key is not found in the map.
func (m *WeilMap[K, V]) Get(key *K) (*V, error) {
	return runtime.ReadCollection[V]([]byte(m.StateTreeKey(key)))
}

// Remove removes the key-value pair associated with the given key from the map.
// Returns the removed value if it existed, or an error if the key was not found.
func (m *WeilMap[K, V]) Remove(key *K) (*V, error) {
	return runtime.DeleteCollectionEntry[V]([]byte(m.StateTreeKey(key)))
}
