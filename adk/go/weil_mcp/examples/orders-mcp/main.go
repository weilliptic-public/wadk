// orders-mcp is an MCP server with two secured, audited tools:
//
//   - lookup_order: any wallet holding Execution or Management in engg.weil
//   - refund_order: only wallets holding Management in engg.weil
//
// Each permitted call returns an audit entry ({"caller", "tool", "args"}) in
// the result's _meta; the calling agent submits it to the auditor applet
// from its own wallet (see shop-assistant).
//
// It listens on http://127.0.0.1:9000/mcp.
package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"sync"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	weilmcp "github.com/weilliptic-public/wadk/adk/go/weil_mcp"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

const (
	listenAddr = "127.0.0.1:9000"
	scope      = "engg.weil" // subgroup "engg" of org "weil"
)

type Order struct {
	ID       string  `json:"id"`
	Item     string  `json:"item"`
	Amount   float64 `json:"amount"`
	Status   string  `json:"status"`
	Refunded bool    `json:"refunded"`
}

// The shop's orders. A real server would read a database.
var (
	mu     sync.Mutex
	orders = map[string]*Order{
		"ORD-1001": {ID: "ORD-1001", Item: "Wireless headphones", Amount: 4999, Status: "delivered damaged"},
		"ORD-1002": {ID: "ORD-1002", Item: "USB-C cable", Amount: 499, Status: "delivered"},
	}
)

type LookupInput struct {
	OrderID string `json:"order_id" jsonschema:"The order ID, e.g. ORD-1001."`
}

type LookupOutput struct {
	Order     Order  `json:"order"`
	CheckedBy string `json:"checked_by"`
}

// lookupOrder runs only after weilmcp.SecuredUser has verified the caller.
func lookupOrder(ctx context.Context, req *mcp.CallToolRequest, in LookupInput) (*mcp.CallToolResult, LookupOutput, error) {
	mu.Lock()
	defer mu.Unlock()
	order, ok := orders[in.OrderID]
	if !ok {
		return nil, LookupOutput{}, fmt.Errorf("no order %s", in.OrderID)
	}
	// weilmcp.Wallet returns the verified caller's wallet address.
	return nil, LookupOutput{Order: *order, CheckedBy: weilmcp.Wallet(ctx)}, nil
}

type RefundInput struct {
	OrderID string `json:"order_id" jsonschema:"The order ID to refund."`
	Reason  string `json:"reason" jsonschema:"Why the order is refunded."`
}

type RefundOutput struct {
	OrderID    string  `json:"order_id"`
	Refunded   float64 `json:"refunded"`
	RefundedBy string  `json:"refunded_by"`
}

// refundOrder runs only after weilmcp.SecuredAdmin has verified the caller.
func refundOrder(ctx context.Context, req *mcp.CallToolRequest, in RefundInput) (*mcp.CallToolResult, RefundOutput, error) {
	mu.Lock()
	defer mu.Unlock()
	order, ok := orders[in.OrderID]
	if !ok {
		return nil, RefundOutput{}, fmt.Errorf("no order %s", in.OrderID)
	}
	if order.Refunded {
		return nil, RefundOutput{}, fmt.Errorf("order %s is already refunded", in.OrderID)
	}
	order.Refunded = true
	log.Printf("refunded %s (%.2f) for %q, approved by wallet %s", order.ID, order.Amount, in.Reason, weilmcp.Wallet(ctx))
	return nil, RefundOutput{OrderID: order.ID, Refunded: order.Amount, RefundedBy: weilmcp.Wallet(ctx)}, nil
}

func main() {
	server := mcp.NewServer(&mcp.Implementation{Name: "orders", Version: "1.0.0"}, nil)

	mcp.AddTool(server,
		&mcp.Tool{Name: "lookup_order", Description: "Returns an order's item, amount and delivery status."},
		weilmcp.SecuredUser(scope, weilmcp.Audit("", lookupOrder))) // Execution or Management in engg.weil

	mcp.AddTool(server,
		&mcp.Tool{Name: "refund_order", Description: "Refunds an order in full."},
		weilmcp.SecuredAdmin(scope, weilmcp.Audit("", refundOrder))) // Management in engg.weil only

	handler := mcp.NewStreamableHTTPHandler(func(*http.Request) *mcp.Server { return server }, nil)

	// Middleware verifies each request's signed X-Wallet-* headers (401 if
	// they fail) and tells the secured tools how to reach Sentinel for the
	// permission check.
	secure := weilmcp.Middleware(handler, secured.WithSentinel(sentinelOptions()...))

	mux := http.NewServeMux()
	mux.Handle("/mcp", secure)
	log.Printf("orders MCP server on http://%s/mcp", listenAddr)
	log.Fatal(http.ListenAndServe(listenAddr, mux))
}
