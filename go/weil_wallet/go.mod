module github.com/weilliptic-inc/contract-sdk/go/weil_wallet

go 1.23.4

require (
	github.com/decred/dcrd/dcrec/secp256k1/v4 v4.3.0
	github.com/tidwall/btree v1.7.0
	github.com/weilliptic-inc/contract-sdk/go/weil_go v0.0.0-20241014181428-709cf9349f24
	github.com/zhangyunhao116/skipmap v0.10.1
)

require github.com/zhangyunhao116/fastrand v0.3.0 // indirect

replace github.com/weilliptic-inc/contract-sdk/go/weil_go => /root/code/contract-sdk/go/weil_go
