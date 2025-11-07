package transaction

import (
	"encoding/hex"
	"encoding/json"

	"github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/tidwall/btree"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
)

type TransactionHeader struct {
	Nonce          uint                  `json:"nonce"`
	PublicKey      string                `json:"public_key"`
	FromAddr       string                `json:"from_addr"`
	ToAddr         string                `json:"to_addr"`
	Signature      *types.Option[string] `json:"signature"`
	WeilpodCounter int                   `json:"weilpod_counter"`
}

func NewTransactionHeader(nonce uint, publicKey string, fromAddr string, toAddr string, weilpodCounter int) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		WeilpodCounter: weilpodCounter,
	}
}

func NewTransactionHeaderWithSignature(nonce uint, publicKey string, fromAddr string, toAddr string, signature string, weilpodCounter int) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		Signature:      types.NewSomeOption(&signature),
		WeilpodCounter: weilpodCounter,
	}
}

func (txn *TransactionHeader) SetSignature(signature string) {
	txn.Signature = types.NewSomeOption(&signature)
}

func (txn *TransactionHeader) ParsedPublicKey() (*secp256k1.PublicKey, error) {
	publicKeyBytes, err := hex.DecodeString(txn.PublicKey)

	if err != nil {
		return nil, err
	}

	return secp256k1.ParsePubKey(publicKeyBytes)
}

type TransactionResult struct {
	Status       string `json:"status"`
	BlockHeight  uint64 `json:"block_height"`
	BatchId      string `json:"batch_id"`
	BatchAuthor  string `json:"batch_author"`
	TxnIdx       uint   `json:"tx_idx"`
	TxnResult    string `json:"txn_result"`
	CreationTime string `json:"creation_time"`
}

type BaseTransaction struct {
	Header TransactionHeader `json:"header"`
}

func NewBaseTransaction(header TransactionHeader) *BaseTransaction {
	return &BaseTransaction{
		Header: header,
	}
}

func ValueToBtreeMap(m map[string]interface{}) btree.Map[string, interface{}] {
	var btreeMap btree.Map[string, interface{}]
	for key, value := range m {
		btreeMap.Set(key, value)
	}
	return btreeMap
}

func BtreeMapToJson(m btree.Map[string, interface{}]) ([]byte, error) {
	vanillaMap := make(map[string]interface{})

	it := m.Iter()
	for it.Next() {
		vanillaMap[it.Key()] = it.Value()
	}

	jsonData, err := json.Marshal(vanillaMap)
	if err != nil {
		return nil, err
	}
	return jsonData, nil
}
