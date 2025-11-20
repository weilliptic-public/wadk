package config

import (
	"encoding/json"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal"
)

//go:wasmimport env get_config
func get_config() int32

type Secrets[T any] struct{}

func NewSecrets[T any]() Secrets[T] {
	return Secrets[T]{}
}

func (s *Secrets[T]) Config() (*T, error) {
	var result T

	ptr := (uintptr)(get_config())
	dataResult := internal.Read(ptr)

	if dataResult.IsErrResult() {
		return nil, *dataResult.TryErrResult()
	}

	dataBytes := dataResult.TryOkResult()
	err := json.Unmarshal(*dataBytes, &result)

	if err != nil {
		return nil, err
	}
	return &result, nil
}
