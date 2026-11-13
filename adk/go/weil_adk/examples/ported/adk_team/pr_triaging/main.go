// Command pr_triaging is adk_pr_triaging_agent (main.py): it works out the
// component of a pull request of OWNER/REPO (PULL_REQUEST_NUMBER), or of up
// to PR_COUNT_TO_PROCESS unassigned ones, and assigns its owner, as the team
// identity, with its secured tools audited and the run traced.
//
//	PULL_REQUEST_NUMBER=123 go run ./examples/ported/adk_team/pr_triaging
package main

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "adk_pr_triaging_app"
	userID  = "adk_pr_triaging_user"
)

func run(ctx context.Context) {
	runner := team.NewRunner(appName, newRootAgent(ctx))
	sessionID, err := runner.NewSession(ctx, userID)
	if err != nil {
		team.Fatalf("%v", err)
	}

	var prompt string
	if prNumber := team.ParseNumberString(pullRequestNumber, 0); prNumber != 0 {
		prompt = fmt.Sprintf("Please triage pull request #%d!", prNumber)
	} else {
		prCount := team.ParseNumberString(prCountToProcess, 10)
		fmt.Printf("No pull request number received. Operating in batch mode (limit: %d).\n", prCount)
		prompt = fmt.Sprintf("Please use 'list_unassigned_pull_requests' to find %d pull"+
			" requests that have nobody assigned, then triage each one according to"+
			" your instructions.", prCount)
	}

	response, err := runner.CallAgent(ctx, userID, sessionID, prompt)
	if err != nil {
		team.Fatalf("%v", err)
	}
	fmt.Printf("<<<< Agent Final Output: %s\n\n", response)
}

func main() {
	loadSettings()
	team.SetupLogger(team.DEBUG)
	ctx := context.Background()

	pr := pullRequestNumber
	if _, set := team.LookupEnv("PULL_REQUEST_NUMBER"); !set {
		pr = "None"
	}
	start := time.Now()
	fmt.Printf("Start triaging %s/%s pull request #%s at %s\n", owner, repo, pr, team.TimeLine(start))
	fmt.Println(strings.Repeat("-", 80))
	run(ctx)
	fmt.Println(strings.Repeat("-", 80))
	end := time.Now()
	fmt.Printf("Triaging finished at %s\n", team.TimeLine(end))
	fmt.Printf("Total script execution time: %.2f seconds\n", end.Sub(start).Seconds())
}
