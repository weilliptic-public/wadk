package transaction

import (
	"encoding/hex"
	"encoding/json"
	"time"

	"github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/google/uuid"
	"github.com/tidwall/btree"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

type TransactionHeader struct {
	Nonce          int                   `json:"nonce"`
	PublicKey      string                `json:"public_key"`
	FromAddr       string                `json:"from_addr"`
	ToAddr         string                `json:"to_addr"`
	Signature      *types.Option[string] `json:"signature"`
	WeilpodCounter int                   `json:"weilpod_counter"`
	CreationTime   int                   `json:"creation_time"`
	// Random UUIDv4. Covered by the signature (see SignExecuteArgs) and
	// mixed into the node's get_txn_id(), so two transactions never collide
	// on id even if nonce happens to match.
	Salt string `json:"salt"`
}

func NewTransactionHeader(nonce int, publicKey string, fromAddr string, toAddr string, weilpodCounter int) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		WeilpodCounter: weilpodCounter,
		CreationTime:   int(time.Now().UnixMilli()),
		Salt:           uuid.NewString(),
	}
}

// NewTransactionHeaderWithSignature rebuilds the wire-format header for
// submission. `salt` must be the same value used when signing (typically
// txn.Header.Salt) — it's covered by the signature, so regenerating it here
// would make the signature fail verification against the node.
func NewTransactionHeaderWithSignature(nonce int, publicKey string, fromAddr string, toAddr string, signature string, weilpodCounter int, salt string) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		Signature:      types.NewSomeOption(&signature),
		WeilpodCounter: weilpodCounter,
		CreationTime:   int(time.Now().UnixMilli()),
		Salt:           salt,
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
	Header *TransactionHeader `json:"header"`
}

func NewBaseTransaction(header *TransactionHeader) *BaseTransaction {
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
