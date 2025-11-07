package api

import (
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/transaction"
)

type Verifier struct {
	Ty string `json:"type"`
}

type UserTransaction struct {
	Ty                 string                `json:"type"`
	ContractAddress    string                `json:"contract_address"`
	ContractMethod     string                `json:"contract_method"`
	ContractInputBytes *types.Option[string] `json:"contract_input_bytes"`
}

type Transaction struct {
	IsXpod    bool                           `json:"is_xpod"`
	TxnHeader *transaction.TransactionHeader `json:"txn_header"`
	Verifier  *Verifier                      `json:"verifier"`
	UserTxn   *UserTransaction               `json:"user_txn"`
}

type SubmitTxnRequest struct {
	Transaction *Transaction `json:"transaction"`
}
