package nonce

import (
	"sync/atomic"

	"github.com/zhangyunhao116/skipmap"
)

type NonceTracker struct {
	Map skipmap.StringMap[*atomic.Uint64]
}

func DefaultNonceTracker() *NonceTracker {
	return &NonceTracker{
		Map: *skipmap.NewString[*atomic.Uint64](),
	}
}

func (tracker *NonceTracker) GetNonce(key string) uint {
	var initialValue atomic.Uint64
	initialValue.Store(1)
	actual, _ := tracker.Map.LoadOrStore(key, &initialValue)

	return uint(actual.Load())
}

//  if the key exists in the NonceTracker , then increment it to expectedNonce + 1
func (tracker *NonceTracker) SetNonce(key string, expectedNonce uint64) {
	actual, loaded := tracker.Map.Load(key)

	if loaded {
		actual.Store(expectedNonce + 1)
	}
}
