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

// OrgContext is the organization a transaction is signed under. It is carried
// in the signed header and surfaced to applets as Runtime::org(), so an applet
// can scope itself to the caller's org without taking one as a method argument.
//
// The wire shape is load-bearing: it must serialize exactly as the node's
// OrgContext does — keys "org"/"subgroup", with "subgroup": null when absent
// rather than omitted. The signature is a SHA-256 over key-sorted JSON that
// both sides rebuild independently, so a renamed or dropped key changes the
// digest and the node rejects the transaction. Note the deliberate absence of
// `omitempty` on Subgroup.
type OrgContext struct {
	Org      string  `json:"org"`
	Subgroup *string `json:"subgroup"`
}

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
	// Organization the signing wallet is acting under, or nil when it has no
	// active org. Covered by the signature (see SignExecuteArgs).
	Org *OrgContext `json:"org"`
}

func NewTransactionHeader(nonce int, publicKey string, fromAddr string, toAddr string, weilpodCounter int, org *OrgContext) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		WeilpodCounter: weilpodCounter,
		CreationTime:   int(time.Now().UnixMilli()),
		Salt:           uuid.NewString(),
		Org:            org,
	}
}

// NewTransactionHeaderWithSignature rebuilds the wire-format header for
// submission. `salt` and `org` must be the same values used when signing
// (typically txn.Header.Salt / txn.Header.Org) — both are covered by the
// signature, so regenerating or dropping either here would make the signature
// fail verification against the node.
func NewTransactionHeaderWithSignature(nonce int, publicKey string, fromAddr string, toAddr string, signature string, weilpodCounter int, salt string, org *OrgContext) *TransactionHeader {
	return &TransactionHeader{
		Nonce:          nonce,
		PublicKey:      publicKey,
		FromAddr:       fromAddr,
		ToAddr:         toAddr,
		Signature:      types.NewSomeOption(&signature),
		WeilpodCounter: weilpodCounter,
		CreationTime:   int(time.Now().UnixMilli()),
		Salt:           salt,
		Org:            org,
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
