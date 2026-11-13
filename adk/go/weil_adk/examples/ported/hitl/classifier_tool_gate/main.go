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

// Command classifier_tool_gate is the Go port of the Python
// hitl/classifier_tool_gate example: a customer database assistant whose
// tool calls first pass a classifier (tool_gate.go); calls it flags as risky
// wait for the user's confirmation. It is served over secured A2A on
// 127.0.0.1:8006.
//
// The classifier runs on Gemini (gemini-3.5-flash), which reads
// GOOGLE_API_KEY or GEMINI_API_KEY; the agent runs on OpenAI's gpt-4o. The
// customers table needs the sqlite3 command line shell (see sqlite.go).
//
// weil: every agent acts under its own identity, resolved by devnet.Agent
// from WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY>
// (a wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	customer_db_assistant  classifier-tool-gate-customer-db-assistant  WEIL_IDENTITY_CLASSIFIER_TOOL_GATE_CUSTOMER_DB_ASSISTANT / WEIL_WALLET_CLASSIFIER_TOOL_GATE_CUSTOMER_DB_ASSISTANT
//
// Grants: run_sql and send_email are SecuredUser("engg.weil"), so
// customer_db_assistant's wallet needs Execution in engg.weil. The
// classifier is a model call inside the before-tool callback, not an agent,
// so it has no identity.
package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"strings"
	"unicode"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/model/gemini"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/hitl/internal/hitlserve"
)

var db *sqliteDB

const seed = `
    CREATE TABLE customers (
        id INTEGER PRIMARY KEY, name TEXT, email TEXT, plan TEXT
    );
    INSERT INTO customers VALUES
        (1, 'Ada Lovelace', 'ada@example.com', 'pro'),
        (2, 'Alan Turing', 'alan@example.com', 'free'),
        (3, 'Grace Hopper', 'grace@example.com', 'pro');
`

type runSQLArgs struct {
	Query string `json:"query"`
}

// runSQL runs one SQLite statement against the `customers` table.
func runSQL(_ agent.Context, args runSQLArgs) (map[string]any, error) {
	rows, rowCount, err := db.Execute(args.Query)
	if sqlErr := (*errSQL)(nil); errors.As(err, &sqlErr) {
		return map[string]any{"error": sqlErr.Error()}, nil
	}
	if err != nil {
		return nil, err
	}
	return map[string]any{"rows": rows, "rows_changed": rowCount}, nil
}

type sendEmailArgs struct {
	To      string `json:"to"`
	Subject string `json:"subject"`
	Body    string `json:"body"`
}

// sendEmail sends an email to a customer.
func sendEmail(_ agent.Context, args sendEmailArgs) (map[string]string, error) {
	return map[string]string{"result": fmt.Sprintf("Sent %s to %s.", pyRepr(args.Subject), args.To)}, nil
}

func createRootAgent(ctx context.Context) (agent.Agent, error) {
	// Any Classifier works here, such as a dedicated decision model.
	classifierModel, err := devnet.E2EModel(gemini.NewModel(ctx, "gemini-3.5-flash", &genai.ClientConfig{}))
	if err != nil {
		return nil, err
	}
	classifier := &LLMClassifier{Model: classifierModel}

	toolGate, err := CreateToolGateCallback(classifier,
		"Would this tool call delete or change data, or contact a customer?",
		WithOnThresholdExceeded("confirm"))
	if err != nil {
		return nil, err
	}

	runSQLTool, err := functiontool.New(functiontool.Config{
		Name:        "run_sql",
		Description: "Runs one SQLite statement against the `customers` table.\n\nThe table has columns id, name, email and plan.",
	}, weiladk.SecuredUser("engg.weil", runSQL))
	if err != nil {
		return nil, err
	}
	sendEmailTool, err := functiontool.New(functiontool.Config{
		Name:        "send_email",
		Description: "Sends an email to a customer.",
	}, weiladk.SecuredUser("engg.weil", sendEmail))
	if err != nil {
		return nil, err
	}

	tools := []tool.Tool{runSQLTool, sendEmailTool}
	tools = append(tools, weiladk.PensieveTools()...) // setup_secure_a2a(pensieve=True)

	return weiladk.NewLLMAgent(hitlserve.Identity(ctx, "classifier-tool-gate-customer-db-assistant"), llmagent.Config{
		Name:  "customer_db_assistant",
		Model: hitlserve.GPT4o(ctx),
		Instruction: "You help support staff look up and manage customers. Use `run_sql` to" +
			" query or change the `customers` table, and `send_email` to email a" +
			" customer. Report tool results to the user.",
		Tools:               tools,
		BeforeToolCallbacks: []llmagent.BeforeToolCallback{toolGate},
	})
}

func main() {
	var err error
	if db, err = openSQLite(seed); err != nil {
		log.Fatalf("Failed to open database: %v", err)
	}
	defer db.Close()

	rootAgent, err := createRootAgent(context.Background())
	if err != nil {
		log.Fatalf("Failed to create agent: %v", err)
	}
	hitlserve.Main(rootAgent, "127.0.0.1", 8006)
}

// pyRepr is Python's repr of a str, which the Python tool's {subject!r}
// prints: single-quoted unless the text has a ' and no ".
func pyRepr(s string) string {
	quote := '\''
	if strings.ContainsRune(s, '\'') && !strings.ContainsRune(s, '"') {
		quote = '"'
	}
	var b strings.Builder
	b.WriteRune(quote)
	for _, r := range s {
		switch {
		case r == quote || r == '\\':
			b.WriteRune('\\')
			b.WriteRune(r)
		case r == '\n':
			b.WriteString(`\n`)
		case r == '\r':
			b.WriteString(`\r`)
		case r == '\t':
			b.WriteString(`\t`)
		case r < 0x20 || r == 0x7f:
			fmt.Fprintf(&b, `\x%02x`, r)
		case !unicode.IsPrint(r) && r != ' ':
			switch {
			case r <= 0xff:
				fmt.Fprintf(&b, `\x%02x`, r)
			case r <= 0xffff:
				fmt.Fprintf(&b, `\u%04x`, r)
			default:
				fmt.Fprintf(&b, `\U%08x`, r)
			}
		default:
			b.WriteRune(r)
		}
	}
	b.WriteRune(quote)
	return b.String()
}
