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

package main

import (
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
)

// reimburse reimburses the amount of money to the employee.
var reimburse functiontool.Func[reimburseArgs, map[string]string] = weiladk.SecuredUser("engg.weil",
	func(_ agent.Context, _ reimburseArgs) (map[string]string, error) {
		return map[string]string{
			"status": "ok",
		}, nil
	})

type reimburseArgs struct {
	Purpose string  `json:"purpose"`
	Amount  float64 `json:"amount"`
}

// askForApproval asks for approval for the reimbursement.
var askForApproval functiontool.Func[approvalArgs, map[string]any] = weiladk.SecuredUser("engg.weil",
	func(_ agent.Context, args approvalArgs) (map[string]any, error) {
		return map[string]any{
			"status":   "pending",
			"amount":   args.Amount,
			"ticketId": "reimbursement-ticket-001",
		}, nil
	})

type approvalArgs struct {
	Purpose string  `json:"purpose"`
	Amount  float64 `json:"amount"`
}
