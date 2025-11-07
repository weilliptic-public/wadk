package contract

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-public/jsonmap"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/collections"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

type CollectionsContractState struct {
	AString  string                                   `json:"string"`
	AInt8    int8                                     `json:"int8"`
	AMap     collections.WeilMap[string, string]      `json:"map"`
	AMap2Set collections.WeilMap[string, jsonmap.Map] `json:"map2sets"`
}

func NewCollectionsContractState() (*CollectionsContractState, error) {
	return &CollectionsContractState{
		AString:  "a string",
		AInt8:    int8(4),
		AMap:     *collections.NewWeilMap[string, string](*collections.NewWeilId(0)),
		AMap2Set: *collections.NewWeilMap[string, jsonmap.Map](*collections.NewWeilId(1)),
	}, nil
}

// query
func (obj *CollectionsContractState) Get() {
	runtime.DebugLog(fmt.Sprintf("vals %v", obj))
	aKey := "the key"
	values, err := obj.AMap2Set.Get(&aKey)
	if err != nil {
		runtime.DebugLog("Set err 2")
		values = jsonmap.New()
	}
	runtime.DebugLog(fmt.Sprintf("vals1 %v", values))
}

// mutate
func (obj *CollectionsContractState) Set() {
	aKey := "the key"
	aVal := "the value"
	runtime.DebugLog("Set 1")
	_, err := obj.AMap.Get(&aKey)
	if err != nil {
		runtime.DebugLog("Set err1")
	}
	obj.AMap.Insert(&aKey, &aVal)
	runtime.DebugLog("Set 2")

	values, err := obj.AMap2Set.Get(&aKey)
	if err != nil {
		values = jsonmap.New()
	}
	runtime.DebugLog("Set2 err 2.5")
	if err == nil {
		runtime.DebugLog("Set2 err 3")
	}

	runtime.DebugLog(fmt.Sprintf("vals1a %v", values))
	values.Set(fmt.Sprintf("%d", values.Len()+1), aVal)
	values.Set(fmt.Sprintf("%d", values.Len()+1), aVal)
	runtime.DebugLog(fmt.Sprintf("vals1b %v", values))
	runtime.DebugLog("Set2 err 3")
	valsSer, err := json.Marshal(values)
	if err != nil {
		runtime.DebugLog("Set2 err 4")
	}

	runtime.DebugLog(fmt.Sprintf("vals1ser %s", valsSer))
	obj.AMap2Set.Insert(&aKey, values)

}
