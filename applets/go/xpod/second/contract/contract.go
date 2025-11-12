package contract

import "github.com/weilliptic-public/wadk/adk/go/weil_go/collections"

// @contract_state
type SecondContractState struct {
	Map collections.WeilMap[string, []int8]
}

func NewSecondContractState() (*SecondContractState, error) {
	return &SecondContractState{
		Map: collections.WeilMap[string, []int8](*collections.NewWeilMap[string, []uint8](*collections.NewWeilId(0))),
	}, nil
}

// query
func (obj *SecondContractState) GetList(id string) (*[]int8, error) {
	val, err := obj.Map.Get(&id)

	if err != nil {
		return nil, err
	}

	return val, nil
}

// mutate
func (obj *SecondContractState) SetVal(id string, val int8) []int8 {
	list, err := obj.Map.Get(&id)

	if err != nil {
		obj.Map.Insert(&id, &[]int8{val})

		return []int8{val}
	}

	l := *list
	l = append(l, val)
	obj.Map.Insert(&id, &l)

	return l
}
