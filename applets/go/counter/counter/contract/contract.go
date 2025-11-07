package contract

type CounterContractState struct {
    Val uint32 `json:"inner"`
}

func NewCounterContractState() (*CounterContractState, error) {
    return &CounterContractState {
	Val: 0,
    }, nil
}

// query
func (obj *CounterContractState) GetCount() uint32 {
     return obj.Val
}

// mutate
func (obj *CounterContractState) Increment() {
    obj.Val++
}

// mutate
func (obj *CounterContractState) SetValue(val uint32) {
    obj.Val = val
}
