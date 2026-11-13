module github.com/weilliptic-public/wadk/adk/go/weil_mcp

go 1.25.0

replace (
	github.com/weilliptic-public/wadk/adk/go/weil_go => ../weil_go
	github.com/weilliptic-public/wadk/adk/go/weil_wallet => ../weil_wallet
)

require (
	github.com/modelcontextprotocol/go-sdk v1.8.0
	github.com/weilliptic-public/wadk/adk/go/weil_wallet v0.0.0-00010101000000-000000000000
)

require (
	github.com/decred/dcrd/dcrec/secp256k1/v4 v4.3.0 // indirect
	github.com/google/jsonschema-go v0.4.3 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/segmentio/asm v1.1.3 // indirect
	github.com/segmentio/encoding v0.5.4 // indirect
	github.com/tidwall/btree v1.7.0 // indirect
	github.com/weilliptic-public/wadk/adk/go/weil_go v0.0.0-20241014181428-709cf9349f24 // indirect
	github.com/yosida95/uritemplate/v3 v3.0.2 // indirect
	github.com/zhangyunhao116/fastrand v0.3.0 // indirect
	github.com/zhangyunhao116/skipmap v0.10.1 // indirect
	golang.org/x/oauth2 v0.35.0 // indirect
	golang.org/x/sync v0.20.0 // indirect
	golang.org/x/sys v0.41.0 // indirect
	golang.org/x/time v0.15.0 // indirect
)
