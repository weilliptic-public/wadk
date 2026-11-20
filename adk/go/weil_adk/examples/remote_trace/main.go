// remote_trace: a planner calling a remote Weil agent while sharing one
// trace — the Go version of the Python SDK's
// examples/tracer/langchain_remote_trace.py.
//
//	go run ./examples/remote_trace
//
// The example serves the fulfillment agent (agent B) over HTTP
// (weiladk.AgentHTTPHandler), then runs the planner (agent A). The planner
// does two local tool calls, calls the fulfillment agent with
// RemoteAgent.Call, then answers. The remote call signs the planner's trace
// context, so the fulfillment agent's RecordToolCall nodes go into the
// planner's trace, under the tool call that made the call. Only the planner
// creates, indexes and publishes the trace. At the end the example reads the
// trace back with get_trace and prints it: the fulfillment agent's three
// nodes hang under the planner's ask_fulfillment_agent call.
//
// Each agent acts under its own identity (devnet.Agent), set by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent        identity      <KEY>
//	planner      planner       PLANNER
//	fulfillment  fulfillment   FULFILLMENT
//
// No grants are needed. The planner's trace is published to its org's
// publisher, so give it an identity linked to an org.
//
// Environment: API_KEY (or ANTHROPIC_API_KEY), WEIL_SENTINEL_HOST (default
// https://sentinel.weilliptic.ai).
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

const fulfillmentAddr = "127.0.0.1:8123"

// checkFulfillment is the fulfillment agent's method. Its work is fixed, so
// the shape of the trace is easy to inspect.
func checkFulfillment(ctx context.Context, args []any, _ map[string]any) (any, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("check_fulfillment takes the order JSON")
	}
	orderJSON, _ := args[0].(string)
	var order struct {
		SKU            string `json:"sku"`
		Quantity       int    `json:"quantity"`
		DestinationZip string `json:"destination_zip"`
	}
	if err := json.Unmarshal([]byte(orderJSON), &order); err != nil {
		return nil, err
	}
	// weil: each call adds a node to the caller's trace (Python's
	// weil_ai.tracer.record_tool_call).
	weiladk.RecordToolCall(ctx, "fulfillment.parse_order",
		map[string]any{"order_json": orderJSON},
		map[string]any{"sku": order.SKU, "destination_zip": order.DestinationZip})
	weiladk.RecordToolCall(ctx, "fulfillment.reserve_inventory",
		map[string]any{"sku": order.SKU, "quantity": order.Quantity},
		map[string]any{"reservation_id": "resv-coffee-001", "reserved": true})
	weiladk.RecordToolCall(ctx, "fulfillment.rate_shipper",
		map[string]any{"destination_zip": order.DestinationZip, "service": "priority"},
		map[string]any{"carrier": "WeilShip", "days": 1, "cost": 7.25})
	out, _ := json.Marshal(map[string]any{"reservation_id": "resv-coffee-001", "reserved": true,
		"carrier": "WeilShip", "ship_days": 1, "shipping_cost": 7.25})
	return string(out), nil
}

type requestIn struct {
	UserRequest string `json:"user_request"`
}

type orderIn struct {
	OrderJSON string `json:"order_json"`
}

func normalizeOrder(_ agent.Context, in requestIn) (string, error) {
	out, _ := json.Marshal(map[string]any{"sku": "sku_coffee", "quantity": 2, "destination_zip": "94105",
		"requested_arrival": "tomorrow", "source_request": in.UserRequest})
	return string(out), nil
}

func calculateLocalTax(_ agent.Context, in orderIn) (string, error) {
	var order struct {
		SKU      string `json:"sku"`
		Quantity int    `json:"quantity"`
	}
	if err := json.Unmarshal([]byte(in.OrderJSON), &order); err != nil {
		return "", err
	}
	subtotal := 14.00 * float64(order.Quantity)
	out, _ := json.Marshal(map[string]any{"sku": order.SKU, "quantity": order.Quantity,
		"subtotal": subtotal, "tax": float64(int(subtotal*8.5+0.5)) / 100})
	return string(out), nil
}

func main() {
	ctx := context.Background()
	planner := devnet.Must(devnet.Agent(ctx, "planner"))
	fulfillment := devnet.Must(devnet.Agent(ctx, "fulfillment"))

	// ── Agent B: the fulfillment agent, served over HTTP ─────────────────
	ln, err := net.Listen("tcp", fulfillmentAddr)
	if err != nil {
		log.Fatalf("%s is in use: %v", fulfillmentAddr, err)
	}
	go func() {
		_ = http.Serve(ln, weiladk.AgentHTTPHandler(fulfillment, map[string]weiladk.AgentMethod{
			"check_fulfillment": checkFulfillment,
		}))
	}()
	remote := planner.Remote("http://" + fulfillmentAddr)

	// ── Agent A: the planner ─────────────────────────────────────────────
	var traceID string
	askFulfillment := func(ctx agent.Context, in orderIn) (string, error) {
		// weil: the trace this call carries (Python's current_trace_context()).
		if tc := weiladk.TraceContextOf(ctx); tc != nil {
			traceID = tc.TraceID
		}
		out, err := remote.Call(ctx, "check_fulfillment", []any{in.OrderJSON}, nil)
		if err != nil {
			return "", err
		}
		s, _ := out.(string)
		return s, nil
	}
	tools := []tool.Tool{
		devnet.Must(functiontool.New(functiontool.Config{Name: "normalize_order",
			Description: "Extract the order fields before calling the remote fulfillment agent."}, normalizeOrder)),
		devnet.Must(functiontool.New(functiontool.Config{Name: "calculate_local_tax",
			Description: "Calculate local tax for the order before remote fulfillment."}, calculateLocalTax)),
		devnet.Must(functiontool.New(functiontool.Config{Name: "ask_fulfillment_agent",
			Description: "Ask the remote fulfillment agent to reserve stock and rate shipping."}, askFulfillment)),
	}
	root := devnet.Must(weiladk.NewLLMAgent(planner, llmagent.Config{
		Name:  "planner",
		Model: devnet.Claude(),
		Instruction: "You are Agent A, an order planner. For every order request, do these steps in order: " +
			"call normalize_order, call calculate_local_tax with the normalized order JSON, call " +
			"ask_fulfillment_agent with the same normalized order JSON, then answer from the tool results. " +
			"Do not skip the remote fulfillment agent.",
		Tools: tools,
	}))
	tracing := devnet.Must(weiladk.NewPlugin(plugin.Config{}, weiladk.WithTracing(weiladk.TraceConfig{})))

	devnet.Section("planner plans an order")
	devnet.Must(devnet.Run(ctx, devnet.NewRunner("orders", root, runner.PluginConfig{Plugins: []*plugin.Plugin{tracing}}), "order",
		"Plan an order for two bags of sku_coffee to 94105 for arrival tomorrow. Do the local planning "+
			"first, then use the fulfillment agent for reservation and shipping."))

	devnet.Section("the shared trace")
	if traceID == "" {
		log.Fatal("the planner never called the fulfillment agent")
	}
	printTrace(ctx, planner, traceID)
}

// printTrace reads the trace back with get_trace — the tracer renders it
// as a Mermaid flowchart, every node under its parent — and prints it.
func printTrace(ctx context.Context, id *weiladk.Identity, traceID string) {
	var trace any
	for range 10 {
		res, err := id.Invoke(ctx, weiladk.Call{Applet: weiladk.Tracer, Method: "get_trace",
			Args: map[string]any{"trace_id": traceID}})
		if err == nil && res.Decode(&trace) == nil {
			break
		}
		time.Sleep(time.Second)
	}
	fmt.Println("trace", traceID)
	if s, ok := trace.(string); ok {
		fmt.Println(s)
		return
	}
	out, _ := json.MarshalIndent(trace, "", "  ")
	fmt.Println(string(out))
}
