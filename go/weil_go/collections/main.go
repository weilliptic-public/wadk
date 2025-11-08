package collections

import (
	"fmt"
)

type WeilId struct {
	Id uint8 `json:"id"`
}

func NewWeilId(id uint8) *WeilId {
	return &WeilId{id}
}

func (id *WeilId) String() string {
	return fmt.Sprintf("%d", id.Id)
}

type WeilCollections[T any] interface {
	BaseStatePath() string
	StateTreeKey(suffix T) []byte
}
