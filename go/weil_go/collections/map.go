package collections

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

type WeilMap[K comparable, V any] struct {
	StateId WeilId `json:"state_id"`
}

func NewWeilMap[K comparable, V any](stateId WeilId) *WeilMap[K, V] {
	return &WeilMap[K, V]{
		StateId: stateId,
	}
}

// Implement WeilCollection

func (m *WeilMap[K, V]) BaseStatePath() string {
	return m.StateId.String()
}

func (m *WeilMap[K, V]) StateTreeKey(suffix *K) string {
	serializedSuffix, _ := json.Marshal(suffix)
	return fmt.Sprintf("%s_%s", m.BaseStatePath(), string(serializedSuffix))
}

// Implement WeilMap

func (m *WeilMap[K, V]) Insert(key *K, val *V) {
	runtime.WriteCollectionEntry[V]([]byte(m.StateTreeKey(key)), val)
}

func (m *WeilMap[K, V]) Get(key *K) (*V, error) {
	return runtime.ReadCollection[V]([]byte(m.StateTreeKey(key)))
}

func (m *WeilMap[K, V]) Remove(key *K) (*V, error) {
	return runtime.DeleteCollectionEntry[V]([]byte(m.StateTreeKey(key)))
}
