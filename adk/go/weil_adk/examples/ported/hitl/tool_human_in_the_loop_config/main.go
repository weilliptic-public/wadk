// Command tool_human_in_the_loop_config is the Go port of the Python
// hitl/tool_human_in_the_loop_config example: the human_in_loop
// reimbursement agent with its tools kept in tools.go, served over secured
// A2A on 127.0.0.1:8011.
//
// weil: every agent acts under its own identity, resolved by devnet.Agent
// from WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY>
// (a wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	reimbursement_agent  tool-hitl-config-reimbursement-agent  WEIL_IDENTITY_TOOL_HITL_CONFIG_REIMBURSEMENT_AGENT / WEIL_WALLET_TOOL_HITL_CONFIG_REIMBURSEMENT_AGENT
//
// Grants: reimburse and ask_for_approval (tools.go) are
// SecuredUser("engg.weil"), so reimbursement_agent's wallet needs Execution
// in engg.weil.
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

func createRootAgent(ctx context.Context) (agent.Agent, error) {
	reimburseTool, err := functiontool.New(functiontool.Config{
		Name:        "reimburse",
		Description: "Reimburse the amount of money to the employee.",
	}, reimburse)
	if err != nil {
		return nil, err
	}
	approvalTool, err := functiontool.New(functiontool.Config{ // LongRunningFunctionTool(func=ask_for_approval)
		Name:          "ask_for_approval",
		Description:   "Ask for approval for the reimbursement.",
		IsLongRunning: true,
	}, askForApproval)
	if err != nil {
		return nil, err
	}

	tools := []tool.Tool{reimburseTool, approvalTool}
	tools = append(tools, weiladk.PensieveTools()...) // setup_secure_a2a(pensieve=True)

	return weiladk.NewLLMAgent(hitlserve.Identity(ctx, "tool-hitl-config-reimbursement-agent"), llmagent.Config{
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
	hitlserve.Main(rootAgent, "127.0.0.1", 8011)
}
