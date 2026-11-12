// Command issue_formatting is adk_issue_formatting_agent: it checks that
// open issues of OWNER/REPO carry what the bug report / feature request
// templates ask for, and asks authors for what is missing. The Python module
// has no main; this serves the agent with ADK's launcher, as `adk run` /
// `adk web` load the module:
//
//	go run ./examples/ported/adk_team/issue_formatting console
//
// The templates are read, as in Python, from .github/ISSUE_TEMPLATE four
// directories above this file's directory.
package main

import (
	"context"
	"fmt"
	"path/filepath"
	"runtime"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// settings.py
var (
	githubBaseURL = "https://api.github.com"
	owner, repo   string
	isInteractive bool
	github        *team.HTTP
)

func loadSettings() {
	team.LoadDotEnv("")
	token := team.MustGetenv("GITHUB_TOKEN")
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")
	isInteractive = team.IsTrue("INTERACTIVE", "1")
	headers := team.GitHubHeaders(token)
	headers["X-GitHub-Api-Version"] = "2022-11-28"
	github = &team.HTTP{Headers: headers, Timeout: 60 * time.Second}
}

func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "message": msg}
}

type issueCountArgs struct {
	IssueCount int `json:"issue_count"`
}

func listOpenIssues(_ agent.Context, in issueCountArgs) (map[string]any, error) {
	u := githubBaseURL + "/search/issues"
	params := team.Params{
		"q":        fmt.Sprintf("repo:%s/%s is:open is:issue", owner, repo),
		"sort":     "created",
		"order":    "desc",
		"per_page": in.IssueCount,
		"page":     1,
	}
	response, err := github.GetJSON(u, params)
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "issues": team.Get(response, "items")}, nil
}

type issueNumberArgs struct {
	IssueNumber int `json:"issue_number"`
}

func getIssue(_ agent.Context, in issueNumberArgs) (map[string]any, error) {
	u := fmt.Sprintf("%s/repos/%s/%s/issues/%d", githubBaseURL, owner, repo, in.IssueNumber)
	response, err := github.GetJSON(u, nil)
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "issue": response}, nil
}

type addCommentArgs struct {
	IssueNumber int    `json:"issue_number"`
	Comment     string `json:"comment"`
}

func addCommentToIssue(_ agent.Context, in addCommentArgs) (map[string]any, error) {
	fmt.Printf("Attempting to add comment '%s' to issue #%d\n", in.Comment, in.IssueNumber)
	u := fmt.Sprintf("%s/repos/%s/%s/issues/%d/comments", githubBaseURL, owner, repo, in.IssueNumber)
	response, err := github.PostJSON(u, map[string]any{"body": in.Comment})
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "added_comment": response}, nil
}

func listCommentsOnIssue(_ agent.Context, in issueNumberArgs) (map[string]any, error) {
	fmt.Printf("Attempting to list comments on issue #%d\n", in.IssueNumber)
	u := fmt.Sprintf("%s/repos/%s/%s/issues/%d/comments", githubBaseURL, owner, repo, in.IssueNumber)
	response, err := github.GetJSON(u, nil)
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "comments": response}, nil
}

// templatePath is Path(__file__).parent / "../../../../.github/ISSUE_TEMPLATE/<name>".
func templatePath(name string) string {
	_, file, _, _ := runtime.Caller(0)
	return filepath.Dir(file) + "/../../../../.github/ISSUE_TEMPLATE/" + name
}

func approvalInstruction() string {
	if isInteractive {
		return "Ask for user approval or confirmation for adding the comment."
	}
	return "**Do not** wait or ask for user approval or confirmation for adding the" +
		" comment."
}

func main() {
	ctx := context.Background()
	loadSettings()
	bugReportTemplate := team.ReadFile(templatePath("bug_report.md"))
	featureRequestTemplate := team.ReadFile(templatePath("feature_request.md"))

	root := team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "adk_issue_formatting_assistant",
		Description: "Check ADK issue format and content.",
		Instruction: instruction(bugReportTemplate, featureRequestTemplate),
		Tools: []tool.Tool{
			team.SecuredTool("list_open_issues", `List most recent `+"`issue_count`"+` number of open issues in the repo.

Args:
  issue_count: number of issues to return

Returns:
  The status of this request, with a list of issues when successful.`, listOpenIssues),
			team.SecuredTool("get_issue", `Get the details of the specified issue number.

Args:
  issue_number: issue number of the GitHub issue.

Returns:
  The status of this request, with the issue details when successful.`, getIssue),
			team.SecuredTool("add_comment_to_issue", `Add the specified comment to the given issue number.

Args:
  issue_number: issue number of the GitHub issue
  comment: comment to add

Returns:
  The status of this request, with the applied comment when successful.`, addCommentToIssue),
			team.SecuredTool("list_comments_on_issue", `List all comments on the given issue number.

Args:
  issue_number: issue number of the GitHub issue

Returns:
  The status of this request, with the list of comments when successful.`, listCommentsOnIssue),
		},
	})
	team.Launch(ctx, root)
}
