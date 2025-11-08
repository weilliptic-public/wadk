package collections

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

type __unit__ struct{}

type WeilSet[V comparable] struct {
	InnerMap WeilMap[V, __unit__] `json:"map"`
}

func NewWeilSet[V comparable](stateId WeilId) *WeilSet[V] {
	return &WeilSet[V]{
		InnerMap: WeilMap[V, __unit__]{stateId},
	}
}

// Implement WeilCollection

func (m *WeilSet[V]) BaseStatePath() string {
	return m.InnerMap.BaseStatePath()
}

func (m *WeilSet[V]) StateTreeKey(suffix *V) string {
	serializedSuffix, _ := json.Marshal(suffix)
	return fmt.Sprintf("%s_%s", m.BaseStatePath(), string(serializedSuffix))
}

// Implement WeilSet

func (s *WeilSet[V]) Insert(val *V) {
	runtime.WriteCollectionEntry[V]([]byte(s.StateTreeKey(val)), nil)
}

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
