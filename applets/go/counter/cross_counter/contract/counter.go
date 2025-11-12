package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

type Counter struct {
	ContractId string `json:"contract_id"`
}

func newCounter(contractId string) *Counter {
	return &Counter{
		contractId,
	}
}

func (c *Counter) getCount() (*uint32, error) {
	resp, err := runtime.CallContract[uint32](c.ContractId, "get_count", "")

	if err != nil {
		return nil, err
	} else {
		return resp, nil
	}
}

func (c *Counter) increment() error {
	_, err := runtime.CallContract[interface{}](c.ContractId, "increment", "")

	if err != nil {
		return err
	} else {
		return nil
	}
}
