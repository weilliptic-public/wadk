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

// Command tool_confirmation is the Go port of the Python
// hitl/tool_confirmation example: transfer_funds asks for confirmation
// itself (ctx.RequestConfirmation) when the amount is 100 or more, and
// close_account always requires it (functiontool.Config.RequireConfirmation).
// It is served over secured A2A on 127.0.0.1:8010.
//
// weil: every agent acts under its own identity, resolved by devnet.Agent
// from WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY>
// (a wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	money_transfer_assistant  tool-confirmation-money-transfer-assistant  WEIL_IDENTITY_TOOL_CONFIRMATION_MONEY_TRANSFER_ASSISTANT / WEIL_WALLET_TOOL_CONFIRMATION_MONEY_TRANSFER_ASSISTANT
//
// Grants: transfer_funds and close_account are SecuredUser("engg.weil"), so
// money_transfer_assistant's wallet needs Execution in engg.weil.
package main

import (
	"context"
	"fmt"
	"log"
	"strconv"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/hitl/internal/hitlserve"
)

type transferArgs struct {
	Amount    float64 `json:"amount"`
	Recipient string  `json:"recipient"`
}

// transferFunds transfers funds to a recipient.
func transferFunds(ctx agent.Context, args transferArgs) (map[string]string, error) {
	amount := strconv.FormatFloat(args.Amount, 'f', -1, 64)
	// Only request confirmation for amounts >= 100
	if args.Amount >= 100 {
		if confirmation := ctx.ToolConfirmation(); confirmation == nil {
			if err := ctx.RequestConfirmation(
				fmt.Sprintf("Confirm transfer of $%s to %s.", amount, args.Recipient), nil); err != nil {
				return nil, err
			}
			return map[string]string{
				"error": "This tool call requires confirmation, please approve or reject.",
			}, nil
		} else if !confirmation.Confirmed {
			return map[string]string{"error": "Transfer rejected by user."}, nil
		}
	}

	// Proceed with transfer for amounts < 100 or if confirmed
	return map[string]string{"result": fmt.Sprintf("Successfully transferred $%s to %s.", amount, args.Recipient)}, nil
}

type closeAccountArgs struct {
	AccountID string `json:"account_id"`
}

// closeAccount closes a user account. This is a destructive action.
func closeAccount(_ agent.Context, args closeAccountArgs) (map[string]string, error) {
	// With RequireConfirmation, this function is only called if the user
	// approves.
	return map[string]string{"result": fmt.Sprintf("Account %s closed successfully.", args.AccountID)}, nil
}

func createRootAgent(ctx context.Context) (agent.Agent, error) {
	transferTool, err := functiontool.New(functiontool.Config{
		Name:        "transfer_funds",
		Description: "Transfers funds to a recipient.",
	}, weiladk.SecuredUser("engg.weil", transferFunds))
	if err != nil {
		return nil, err
	}
	closeTool, err := functiontool.New(functiontool.Config{ // FunctionTool(func=close_account, require_confirmation=True)
		Name:                "close_account",
		Description:         "Closes a user account. This is a destructive action.",
		RequireConfirmation: true,
	}, weiladk.SecuredUser("engg.weil", closeAccount))
	if err != nil {
		return nil, err
	}

	tools := []tool.Tool{transferTool, closeTool}
	tools = append(tools, weiladk.PensieveTools()...) // setup_secure_a2a(pensieve=True)

	return weiladk.NewLLMAgent(hitlserve.Identity(ctx, "tool-confirmation-money-transfer-assistant"), llmagent.Config{
		Name:  "money_transfer_assistant",
		Model: hitlserve.GPT4o(ctx),
		Tools: tools,
	})
}

func main() {
	rootAgent, err := createRootAgent(context.Background())
	if err != nil {
		log.Fatalf("Failed to create agent: %v", err)
	}
	hitlserve.Main(rootAgent, "127.0.0.1", 8010)
}
