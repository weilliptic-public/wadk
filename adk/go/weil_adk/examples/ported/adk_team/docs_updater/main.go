// Command docs_updater is the Go port of adk_team's
// adk_documentation/adk_docs_updater (agent.py and main.py): for each
// numbered suggestion of an ADK docs issue, an agent updates the docs and
// opens one pull request.
//
// Usage:
//
//	go run ./examples/ported/adk_team/docs_updater --issue_number 123
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/docs"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "adk_docs_updater"
	userID  = "adk_docs_updater_user"
)

func approvalInstruction() string {
	if docs.IsInteractive {
		return "Ask for user approval or confirmation for creating the pull request."
	}
	return "**Do not** wait or ask for user approval or confirmation for creating the pull request."
}

func newRootAgent(ctx context.Context) agent.Agent {
	instruction := strings.NewReplacer(
		"{CODE_OWNER}", docs.CodeOwner,
		"{CODE_REPO}", docs.CodeRepo,
		"{DOC_OWNER}", docs.DocOwner,
		"{DOC_REPO}", docs.DocRepo,
		"{LOCAL_REPOS_DIR_PATH}", docs.LocalReposDirPath,
		"{APPROVAL_INSTRUCTION}", approvalInstruction(),
	).Replace(rootInstruction)
	return team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "adk_docs_updater",
		Description: "Update the ADK docs based on the code in the ADK Python codebase according to the instructions in the ADK docs issues.",
		Instruction: instruction,
		Tools: []tool.Tool{
			docs.CloneOrPullRepo(),
			docs.ListDirectoryContents(),
			docs.SearchLocalGitRepo(),
			docs.ReadLocalGitRepoFileContent(),
			docs.CreatePullRequestFromChanges(),
			docs.GetIssue(),
		},
	})
}

type result struct {
	num    int
	status string
	err    string
}

func run(issueNumber int) {
	ctx := context.Background()
	root := newRootAgent(ctx)

	// Python calls the secured get_issue directly here; it runs as the
	// agent's identity, as adk_pr_agent's main does.
	resp, err := docs.GetIssueFunc(team.DirectContext(ctx, team.Identity(ctx)),
		docs.IssueArgs{RepoOwner: docs.DocOwner, RepoName: docs.DocRepo, IssueNumber: issueNumber})
	if err != nil {
		team.Fatalf("%v", err)
	}
	if resp["status"] != "success" {
		fmt.Printf("Failed to get issue %d: %s\n\n", issueNumber, team.Repr(resp))
		return
	}
	issue := resp["issue"]
	issueTitle := team.PyStr(getOr(issue, "title", ""))
	issueBody := team.PyStr(getOr(issue, "body", ""))

	// Parse numbered suggestions from issue body
	suggestions := docs.ParseSuggestions(issueBody)
	if len(suggestions) == 0 {
		fmt.Printf("No numbered suggestions found in issue #%d.\n", issueNumber)
		fmt.Println("Falling back to processing the entire issue as a single task.")
		suggestions = []docs.Suggestion{{Num: 1, Text: issueBody}}
	}

	fmt.Printf("Found %d suggestion(s) in issue #%d.\n", len(suggestions), issueNumber)
	fmt.Println(strings.Repeat("=", 80))

	r := team.NewRunner(appName, root)

	var results []result
	for _, s := range suggestions {
		fmt.Printf("\n>>> Processing suggestion #%d...\n", s.Num)
		fmt.Println(strings.Repeat("-", 80))

		// Create a new session for each suggestion to avoid context interference
		sessionID, err := r.NewSession(ctx, userID)
		if err == nil {
			prompt := fmt.Sprintf(`
      Please update the ADK docs according to suggestion #%d from issue #%d.

      Issue title: %s

      Suggestion to process:
      %s

      Note: Focus only on this specific suggestion. Create exactly one pull request for this suggestion.
    `, s.Num, issueNumber, issueTitle, s.Text)
			_, err = r.CallAgent(ctx, userID, sessionID, prompt)
		}
		if err != nil {
			results = append(results, result{num: s.Num, status: "error", err: err.Error()})
			fmt.Printf("<<<< Suggestion #%d failed: %v\n", s.Num, err)
		} else {
			results = append(results, result{num: s.Num, status: "success"})
			fmt.Printf("<<<< Suggestion #%d completed.\n", s.Num)
		}
		fmt.Println(strings.Repeat("-", 80))
	}

	// Print summary
	fmt.Println("\n" + strings.Repeat("=", 80))
	fmt.Println("SUMMARY")
	fmt.Println(strings.Repeat("=", 80))
	var failed []result
	for _, r := range results {
		if r.status == "error" {
			failed = append(failed, r)
		}
	}
	fmt.Printf("Total: %d, Success: %d, Failed: %d\n", len(results), len(results)-len(failed), len(failed))
	if len(failed) > 0 {
		fmt.Println("\nFailed suggestions:")
		for _, r := range failed {
			fmt.Printf("  - Suggestion #%d: %s\n", r.num, r.err)
		}
	}
}

func getOr(m any, key string, def any) any {
	if mm := team.Map(m); mm != nil {
		if v, ok := mm[key]; ok && v != nil {
			return v
		}
	}
	return def
}

func main() {
	team.SetupLogger(team.INFO)
	start := time.Now()
	fmt.Printf("Start creating pull requests to update %s/%s docs according the %s/%s at %s\n",
		docs.DocOwner, docs.DocRepo, docs.CodeOwner, docs.CodeRepo, team.TimeLine(start))
	fmt.Println(strings.Repeat("-", 80))
	fs := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	fs.Usage = func() {
		fmt.Fprintf(fs.Output(), "A script that creates pull requests to update ADK docs.\n\n")
		fs.PrintDefaults()
		fmt.Fprintf(fs.Output(), "\nExample usage: \n\tgo run ./examples/ported/adk_team/docs_updater --issue_number 123\n")
	}
	issueNumber := fs.Int("issue_number", 0, "Answer a specific issue number.")
	_ = fs.Parse(os.Args[1:])
	set := false
	fs.Visit(func(f *flag.Flag) { set = set || f.Name == "issue_number" })
	if !set {
		fs.Usage()
		fmt.Fprintln(os.Stderr, "error: one of the arguments --issue_number is required")
		os.Exit(2)
	}

	if *issueNumber == 0 {
		fmt.Println("Please specify an issue number using --issue_number flag")
	} else {
		run(*issueNumber)
	}
	fmt.Println(strings.Repeat("-", 80))
	end := time.Now()
	fmt.Printf("Updating finished at %s\n", team.TimeLine(end))
	fmt.Printf("Total script execution time: %.2f seconds\n", end.Sub(start).Seconds())
}
