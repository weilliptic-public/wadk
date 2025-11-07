package contract

type CrossCounterContractState struct {
}

func NewCrossCounterContractState() (*CrossCounterContractState, error) {
    return &CrossCounterContractState {}, nil
}

// query
func (obj *CrossCounterContractState) FetchCounterFrom(contractId string) uint32 {
    counter := newCounter(contractId)
    c, err := counter.getCount()
    if err != nil {
        return 0
    }

    return *c
}

// mutate
func (obj *CrossCounterContractState) IncrementCounterOf(contractId string) {
    counter := newCounter(contractId)
    counter.increment()
}
