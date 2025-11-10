package utils

import (
	"encoding/json"
	"fmt"
)

type CollectionKey[T any] struct {
	BasePath string
	suffix   *T
}

func NewCollectionKey[T any](base string, key *T) *CollectionKey[T] {
	return &CollectionKey[T]{
		BasePath: base,
		suffix:   key,
	}
}

func (k *CollectionKey[T]) MarshalJSON() (buffer []byte, err error) {
	serializedKey, err := json.Marshal(k.suffix)

	if err != nil {
		return
	}

	s := fmt.Sprintf("%s_%s", k.BasePath, string(serializedKey))
	buffer, err = json.Marshal(s)

	if err != nil {
		return
	}

	return buffer, nil
}
