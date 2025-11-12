package contract

import (
	"encoding/json"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/collections"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

// contract_state
type FirstContractState struct {
	XpodMapping  collections.WeilMap[string, string] `json:"xpod_mapping"`  // xpod_id -> id
	TotalMapping collections.WeilMap[string, uint32] `json:"total_mapping"` // id -> counter
}

func NewFirstContractState() (*FirstContractState, error) {
	return &FirstContractState{
		XpodMapping:  collections.WeilMap[string, string](*collections.NewWeilMap[string, string](*collections.NewWeilId(0))),
		TotalMapping: collections.WeilMap[string, uint32](*collections.NewWeilMap[string, uint32](*collections.NewWeilId(1))),
	}, nil
}

// query
func (obj *FirstContractState) HealthCheck() string {
	return "Success!"
}

// query
func (obj *FirstContractState) Counter(id string) (*uint32, error) {
	counter, err := obj.TotalMapping.Get(&id)

	if err != nil {
		return nil, err
	}

	return counter, nil
}

// xpod
func (obj *FirstContractState) SetListInSecond(contractId string, id string, val int8) error {
	type Args struct {
		Id  string `json:"id"`
		Val int8   `json:"val"`
	}

	serializedArgs, _ := json.Marshal(Args{
		Id:  id,
		Val: val,
	})

	xpodId, err := runtime.CallXpodContract(contractId, "set_val", string(serializedArgs))

	if err != nil {
		return err
	}

	_, err = obj.TotalMapping.Get(&id)

	if err != nil {
		var initialVal uint32 = 0
		obj.TotalMapping.Insert(&id, &initialVal)
	}

	obj.XpodMapping.Insert(&xpodId, &id)

	return nil
}

// callback(SetListInSecond)
func (obj *FirstContractState) SetListInSecondCallback(xpodId string, result types.Result[[]int8, string]) {
	if result.IsOkResult() {
		id, err := obj.XpodMapping.Get(&xpodId)

		if err != nil {
			return
		}

		counter, err := obj.TotalMapping.Get(id)

		if err != nil {
			return
		}

		var updatedValue uint32 = *counter + 1

		obj.TotalMapping.Insert(id, &updatedValue)
	}
}
