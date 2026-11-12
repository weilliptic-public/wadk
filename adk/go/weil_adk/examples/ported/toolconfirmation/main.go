// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Package main is the Go twin of the Python human_tool_confirmation example
// (python/examples/adk-ported/hitl/human_tool_confirmation): time_off_agent
// handles reimbursement and time off requests, asking a human to confirm
// when needed.
//
//   - reimburse asks for confirmation when the amount is greater than 1000,
//     through functiontool.Config.RequireConfirmationProvider.
//   - request_time_off approves up to 2 days on its own; for more it asks
//     for confirmation with ctx.RequestConfirmation and reads the answer
//     from ctx.ToolConfirmation().
//
// weil: time_off_agent acts under its own identity,
// "toolconfirmation-time-off-agent", resolved by devnet.Agent from
// WEIL_IDENTITY_TOOLCONFIRMATION_TIME_OFF_AGENT (an Agent Registry identity)
// or WEIL_WALLET_TOOLCONFIRMATION_TIME_OFF_AGENT (a wallet file), else
// ~/.weilliptic/agents/toolconfirmation-time-off-agent.wc. Both tools are
// SecuredUser("engg.weil"): they run once that wallet holds Execution or
// Management in engg.weil.
//
//	go run .                 # serve time_off_agent over secured A2A on 127.0.0.1:8008 (Python's a2a_app)
//	go run . console|web ... # run it locally with the ADK launcher (Python's adk run / adk web)
//
// Environment: OPENAI_API_KEY, WEIL_SENTINEL_HOST and the identity above.
package main

import (
	"context"
	"encoding/json"
	"log"
	"net"
	"net/http"
	"os"
	"time"

	"google.golang.org/genai"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/cmd/launcher"
	"google.golang.org/adk/v2/cmd/launcher/full"
	"google.golang.org/adk/v2/model/openaimodel" // model: Python's LiteLlm("openai/gpt-4o")
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

const listenAddr = "127.0.0.1:8008" // weil: Python's to_secure_a2a(host="127.0.0.1", port=8008)

// ReimburseArgs are the arguments of the reimburse tool.
type ReimburseArgs struct {
	Amount float64 `json:"amount"`
}

// reimburse reimburses the employee for the given amount.
func reimburse(_ agent.Context, _ ReimburseArgs) (map[string]any, error) {
	return map[string]any{"status": "ok"}, nil
}

// confirmationThreshold returns true if the amount is greater than 1000.
func confirmationThreshold(args ReimburseArgs) bool {
	return args.Amount > 1000
}

// RequestTimeOffArgs are the arguments of the request_time_off tool.
type RequestTimeOffArgs struct {
	Days int `json:"days"`
}

// requestTimeOff requests day off for the employee.
func requestTimeOff(ctx agent.Context, args RequestTimeOffArgs) (map[string]any, error) {
	days := args.Days
	if days <= 0 {
		return map[string]any{"status": "Invalid days to request."}, nil
	}

	if days <= 2 {
		return map[string]any{
			"status":        "ok",
			"approved_days": days,
		}, nil
	}

	confirmation := ctx.ToolConfirmation()
	if confirmation == nil {
		err := ctx.RequestConfirmation(
			"Please approve or reject the tool call request_time_off() by"+
				" responding with a FunctionResponse with an expected"+
				" ToolConfirmation payload.",
			map[string]any{
				"approved_days": 0,
			})
		if err != nil {
			return nil, err
		}
		return map[string]any{"status": "Manager approval is required."}, nil
	}

	if !confirmation.Confirmed {
		return map[string]any{"status": "The time off request is rejected.", "approved_days": 0}, nil
	}

	// The payload is optional: a client may confirm with just
	// {"confirmed": true}, which approves the days that were asked for. When
	// the payload is present it narrows the approval.
	approvedDays := min(approvedDaysIn(confirmation.Payload, days), days)
	if approvedDays == 0 {
		return map[string]any{"status": "The time off request is rejected.", "approved_days": 0}, nil
	}
	return map[string]any{
		"status":        "ok",
		"approved_days": approvedDays,
	}, nil
}

// approvedDaysIn reads payload["approved_days"], or def when the payload or
// the field is missing (Python: payload.get('approved_days', days)).
func approvedDaysIn(payload any, def int) int {
	if payload == nil {
		return def
	}
	var p struct {
		ApprovedDays *float64 `json:"approved_days"`
	}
	b, err := json.Marshal(payload)
	if err != nil || json.Unmarshal(b, &p) != nil || p.ApprovedDays == nil {
		return def
	}
	return int(*p.ApprovedDays)
}

func main() {
	ctx := context.Background()

	model, err := devnet.E2EModel(openaimodel.NewModel(ctx, "gpt-4o", &openaimodel.ClientConfig{
		APIKey: os.Getenv("OPENAI_API_KEY"),
	}))
	if err != nil {
		log.Fatalf("Failed to create model: %v", err)
	}

	// Set RequireConfirmation to true or RequireConfirmationProvider to a
	// function to require user confirmation for the tool call. This is an
	// easier way to get user confirmation if the tool just need a boolean
	// confirmation.
	reimburseTool, err := functiontool.New(functiontool.Config{
		Name:                        "reimburse",
		Description:                 "Reimburse the employee for the given amount.",
		RequireConfirmationProvider: confirmationThreshold,
	}, weiladk.SecuredUser("engg.weil", reimburse))
	if err != nil {
		log.Fatalf("Failed to create reimburse tool: %v", err)
	}
	timeOffTool, err := functiontool.New(functiontool.Config{
		Name:        "request_time_off",
		Description: "Request day off for the employee.",
	}, weiladk.SecuredUser("engg.weil", requestTimeOff))
	if err != nil {
		log.Fatalf("Failed to create request_time_off tool: %v", err)
	}

	rootAgent, err := weiladk.NewLLMAgent(devnet.Must(devnet.Agent(ctx, "toolconfirmation-time-off-agent")), llmagent.Config{
		Name:  "time_off_agent",
		Model: model,
		Instruction: `
    You are a helpful assistant that can help employees with reimbursement and time off requests.
    - Use the ` + "`reimburse`" + ` tool for reimbursement requests.
    - Use the ` + "`request_time_off`" + ` tool for time off requests.
    - Prioritize using tools to fulfill the user's request.
    - Always respond to the user with the tool results.
    `,
		// weil: setup_secure_a2a() adds Pensieve to the root.
		Tools:                 append([]tool.Tool{reimburseTool, timeOffTool}, weiladk.PensieveTools()...),
		GenerateContentConfig: &genai.GenerateContentConfig{Temperature: genai.Ptr[float32](0.1)},
	})
	if err != nil {
		log.Fatalf("Failed to create time_off_agent: %v", err)
	}

	if len(os.Args) < 2 { // weil: Python's a2a_app
		serve(rootAgent)
		return
	}

	secure, err := weiladk.NewPlugin(plugin.Config{}, // weil: Python's setup_secure_a2a() on the local runner
		weiladk.WithSecureA2A(weiladk.SecureA2AConfig{}),
		weiladk.WithFlushAfterRun(time.Minute))
	if err != nil {
		log.Fatalf("Failed to create plugin: %v", err)
	}

	config := &launcher.Config{
		AgentLoader:  agent.NewSingleLoader(rootAgent),
		PluginConfig: runner.PluginConfig{Plugins: []*plugin.Plugin{secure}},
	}

	l := full.NewLauncher()
	if err = l.Execute(ctx, config, os.Args[1:]); err != nil {
		log.Fatalf("Run failed: %v\n\n%s", err, l.CommandLineSyntax())
	}
}

// serve exposes the agent over secured A2A on listenAddr, as Python's
// to_secure_a2a does: no service gate, any verified caller is served.
// weiladk.ToSecureA2A builds to_a2a's card and installs the agent and
// secured-tool audits and the run traces.
func serve(served agent.Agent) {
	handler, err := weiladk.ToSecureA2A(served, "http://"+listenAddr)
	if err != nil {
		log.Fatalf("Failed to create secure A2A handler: %v", err)
	}
	listener, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("Failed to listen on %s: %v", listenAddr, err)
	}
	log.Printf("%s serving secured A2A on http://%s", served.Name(), listenAddr)
	log.Fatal(http.Serve(listener, handler))
}
