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

// Command request_input_tool is the Go port of the Python
// hitl/request_input_tool example: an IT support agent that asks the user
// for missing ticket details through the adk_request_input tool before
// filing the ticket. It is served over secured A2A on 127.0.0.1:8009.
//
// ADK-Go v2.5 has no LlmAgent request_input tool (only the workflow
// package's request-input events), so requestInput below rebuilds Python's
// google.adk.tools.request_input the way Python defines it: a long-running
// function tool named adk_request_input that returns nothing, which pauses
// the run until the client answers that call with a FunctionResponse.
//
// weil: every agent acts under its own identity, resolved by devnet.Agent
// from WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY>
// (a wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	support_assistant_agent  request-input-tool-support-assistant-agent  WEIL_IDENTITY_REQUEST_INPUT_TOOL_SUPPORT_ASSISTANT_AGENT / WEIL_WALLET_REQUEST_INPUT_TOOL_SUPPORT_ASSISTANT_AGENT
//
// Grants: create_support_ticket is SecuredUser("engg.weil"), so
// support_assistant_agent's wallet needs Execution in engg.weil.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"

	"github.com/google/jsonschema-go/jsonschema"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/hitl/internal/hitlserve"
)

// SupportTicket is the details of the IT support ticket to be created.
type SupportTicket struct {
	Title       string `json:"title" jsonschema:"A brief summary of the issue."`
	Description string `json:"description" jsonschema:"Detailed explanation of the problem."`
	Priority    string `json:"priority,omitempty" jsonschema:"Ticket priority: LOW, MEDIUM, HIGH, or CRITICAL."`
	Category    string `json:"category" jsonschema:"Issue category, e.g., billing, technical, account, or database."`
}

type createTicketArgs struct {
	Ticket SupportTicket `json:"ticket" jsonschema:"Details of the IT support ticket to be created."`
}

// createSupportTicket creates a support ticket in the IT ticketing system.
func createSupportTicket(_ agent.Context, args createTicketArgs) (map[string]string, error) {
	ticket := args.Ticket
	if ticket.Priority == "" {
		ticket.Priority = "MEDIUM"
	}
	return map[string]string{
		"status": "success",
		"message": fmt.Sprintf("Successfully created ticket '%s' [Category: %s, Priority: %s].",
			ticket.Title, ticket.Category, ticket.Priority),
		"ticket_id": "INC-98471",
	}, nil
}

// ticketSchema is createTicketArgs' schema with priority defaulting to
// MEDIUM, as the Pydantic model declares.
func ticketSchema() (*jsonschema.Schema, error) {
	s, err := jsonschema.For[createTicketArgs](nil)
	if err != nil {
		return nil, err
	}
	s.Properties["ticket"].Properties["priority"].Default = json.RawMessage(`"MEDIUM"`)
	return s, nil
}

type requestInputArgs struct {
	Message        string         `json:"message"`
	ResponseSchema map[string]any `json:"response_schema,omitempty"`
}

// requestInput is Python's google.adk.tools.request_input: ask the user a
// question and wait for their response. Returning nothing from a
// long-running tool pauses the run for the user's input.
func requestInput() (tool.Tool, error) {
	return functiontool.New(functiontool.Config{
		Name: "adk_request_input",
		Description: `Ask the user a question and wait for their response.

Use this when you need clarification or additional information before
proceeding.

Args:
  message: The question or prompt to display to the user.
  response_schema: JSON Schema describing the expected response format. Use
    {"type": "string"} for free-text, {"type": "boolean"} for
    yes/no, or a structured object schema for complex input.

Returns:
  None. Long-running tools return None to signal that the execution should
  pause and wait for user input.`,
		IsLongRunning: true,
	}, func(_ agent.Context, args requestInputArgs) (map[string]any, error) {
		log.Printf("request_input called with message: %s", args.Message)
		// A nil result triggers the long-running tool interruption mechanism.
		return nil, nil
	})
}

// validateRequestInput is the check Python's request_input makes on resume:
// the user's response to an adk_request_input call must match the
// response_schema the model asked for (Python unwraps {"result": value}
// first), or the run fails with "Validation failed for interrupt <id>".
func validateRequestInput(_ agent.Context, req *model.LLMRequest) (*model.LLMResponse, error) {
	if len(req.Contents) == 0 {
		return nil, nil
	}
	schemas := map[string]map[string]any{} // call ID → response_schema
	for _, c := range req.Contents {
		for _, p := range c.Parts {
			if fc := p.FunctionCall; fc != nil && fc.Name == "adk_request_input" {
				if rs, ok := fc.Args["response_schema"].(map[string]any); ok {
					schemas[fc.ID] = rs
				}
			}
		}
	}
	// Only the newest turn carries a response the run has not yet checked.
	for _, p := range req.Contents[len(req.Contents)-1].Parts {
		fr := p.FunctionResponse
		if fr == nil || fr.Name != "adk_request_input" || schemas[fr.ID] == nil {
			continue
		}
		var value any = fr.Response
		if v, ok := fr.Response["result"]; ok {
			value = v
		}
		raw, err := json.Marshal(schemas[fr.ID])
		if err != nil {
			return nil, err
		}
		var schema jsonschema.Schema
		if err := json.Unmarshal(raw, &schema); err != nil {
			return nil, fmt.Errorf("Validation failed for interrupt %s: bad response_schema: %w", fr.ID, err)
		}
		resolved, err := schema.Resolve(nil)
		if err != nil {
			return nil, fmt.Errorf("Validation failed for interrupt %s: %w", fr.ID, err)
		}
		if err := resolved.Validate(value); err != nil {
			return nil, fmt.Errorf("Validation failed for interrupt %s: %w", fr.ID, err)
		}
	}
	return nil, nil
}

func createRootAgent(ctx context.Context) (agent.Agent, error) {
	schema, err := ticketSchema()
	if err != nil {
		return nil, err
	}
	ticketTool, err := functiontool.New(functiontool.Config{
		Name:        "create_support_ticket",
		Description: "Create a support ticket in the IT ticketing system.",
		InputSchema: schema,
	}, weiladk.SecuredUser("engg.weil", createSupportTicket))
	if err != nil {
		return nil, err
	}
	requestInputTool, err := requestInput()
	if err != nil {
		return nil, err
	}

	tools := []tool.Tool{ticketTool, requestInputTool}
	tools = append(tools, weiladk.PensieveTools()...) // setup_secure_a2a(pensieve=True)

	return weiladk.NewLLMAgent(hitlserve.Identity(ctx, "request-input-tool-support-assistant-agent"), llmagent.Config{
		Name:  "support_assistant_agent",
		Model: hitlserve.GPT4o(ctx),
		Instruction: `
      You are a helpful IT support assistant responsible for creating support tickets.
      When the user requests to create or file a ticket:
      1. Identify which ticket details (title, description, priority, category) are already provided in the conversation.
      2. If any mandatory details are missing, call the ` + "`adk_request_input`" + ` tool.
      3. When calling ` + "`adk_request_input`" + `, you must construct a dynamic JSON ` + "`response_schema`" + ` (type: "object") that ONLY requests the missing details, and specify a helpful message explaining what is needed.
      4. Once all details are gathered, call ` + "`create_support_ticket`" + ` with the complete SupportTicket details.
    `,
		Tools:                 tools,
		GenerateContentConfig: &genai.GenerateContentConfig{Temperature: genai.Ptr[float32](0.1)},
		BeforeModelCallbacks:  []llmagent.BeforeModelCallback{validateRequestInput},
	})
}

func main() {
	rootAgent, err := createRootAgent(context.Background())
	if err != nil {
		log.Fatalf("Failed to create agent: %v", err)
	}
	hitlserve.Main(rootAgent, "127.0.0.1", 8009)
}
