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

// Command human_in_loop is the Go port of the Python hitl/human_in_loop
// example: a reimbursement agent whose ask_for_approval tool is
// long-running, so the run pauses until the manager's answer arrives as a
// FunctionResponse. It is served over secured A2A on 127.0.0.1:8007.
//
// weil: every agent acts under its own identity, resolved by devnet.Agent
// from WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY>
// (a wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	reimbursement_agent  human-in-loop-reimbursement-agent  WEIL_IDENTITY_HUMAN_IN_LOOP_REIMBURSEMENT_AGENT / WEIL_WALLET_HUMAN_IN_LOOP_REIMBURSEMENT_AGENT
//
// Grants: reimburse and ask_for_approval are SecuredUser("engg.weil"), so
// reimbursement_agent's wallet needs Execution in engg.weil.
package main

import (
	"context"
	"log"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/hitl/internal/hitlserve"
)

type reimburseArgs struct {
	Purpose string  `json:"purpose"`
	Amount  float64 `json:"amount"`
}

// reimburse reimburses the amount of money to the employee.
func reimburse(_ agent.Context, _ reimburseArgs) (map[string]string, error) {
	return map[string]string{
		"status": "ok",
	}, nil
}

type approvalArgs struct {
	Purpose string  `json:"purpose"`
	Amount  float64 `json:"amount"`
}

// askForApproval asks for approval for the reimbursement.
func askForApproval(_ agent.Context, args approvalArgs) (map[string]any, error) {
	return map[string]any{
		"status":   "pending",
		"amount":   args.Amount,
		"ticketId": "reimbursement-ticket-001",
	}, nil
}

func createRootAgent(ctx context.Context) (agent.Agent, error) {
	reimburseTool, err := functiontool.New(functiontool.Config{
		Name:        "reimburse",
		Description: "Reimburse the amount of money to the employee.",
	}, weiladk.SecuredUser("engg.weil", reimburse))
	if err != nil {
		return nil, err
	}
	approvalTool, err := functiontool.New(functiontool.Config{ // LongRunningFunctionTool(func=ask_for_approval)
		Name:          "ask_for_approval",
		Description:   "Ask for approval for the reimbursement.",
		IsLongRunning: true,
	}, weiladk.SecuredUser("engg.weil", askForApproval))
	if err != nil {
		return nil, err
	}

	tools := []tool.Tool{reimburseTool, approvalTool}
	tools = append(tools, weiladk.PensieveTools()...) // setup_secure_a2a(pensieve=True)

	return weiladk.NewLLMAgent(hitlserve.Identity(ctx, "human-in-loop-reimbursement-agent"), llmagent.Config{
		Name:  "reimbursement_agent",
		Model: hitlserve.GPT4o(ctx),
		Instruction: `
      You are an agent whose job is to handle the reimbursement process for
      the employees. If the amount is less than $100, you will automatically
      approve the reimbursement.

      If the amount is greater than $100, you will
      ask for approval from the manager. If the manager approves, you will
      call reimburse() to reimburse the amount to the employee. If the manager
      rejects, you will inform the employee of the rejection.
`,
		Tools:                 tools,
		GenerateContentConfig: &genai.GenerateContentConfig{Temperature: genai.Ptr[float32](0.1)},
	})
}

func main() {
	rootAgent, err := createRootAgent(context.Background())
	if err != nil {
		log.Fatalf("Failed to create agent: %v", err)
	}
	hitlserve.Main(rootAgent, "127.0.0.1", 8007)
}
