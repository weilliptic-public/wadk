// Command pr is adk_pr_agent (main.py): it fetches pull request #1422 of
// OWNER/REPO through the secured get_github_pr_info_http tool, called
// directly as the team identity, and has github_pr_agent write a description
// for it.
//
//	go run ./examples/ported/adk_team/pr
package main

import (
	"context"
	"fmt"
	"net/http"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

var (
	githubToken string
	owner, repo string
)

type prArgs struct {
	PRNumber int `json:"pr_number"`
}

// getGitHubPRInfoHTTP is get_github_pr_info_http: the PR message, or nil
// (Python's None) when the PR cannot be fetched.
func getGitHubPRInfoHTTP(_ agent.Context, in prArgs) (any, error) {
	const baseURL = "https://api.github.com"
	gh := &team.HTTP{Headers: map[string]string{
		"Accept":               "application/vnd.github+json",
		"Authorization":        "Bearer " + githubToken,
		"X-GitHub-Api-Version": "2022-11-28",
	}}
	none := map[string]any{"result": nil}

	var prMessage strings.Builder

	// --- 1. Get main PR details ---
	prURL := fmt.Sprintf("%s/repos/%s/%s/pulls/%d", baseURL, owner, repo, in.PRNumber)
	fmt.Printf("Fetching PR details from: %s\n", prURL)
	resp, err := gh.Do(http.MethodGet, prURL, nil, nil, nil)
	if err != nil {
		fmt.Printf("Network or request error fetching PR details: %v\n", err)
		return none, nil
	}
	if resp.RaiseForStatus() != nil {
		fmt.Printf("HTTP Error fetching PR details: %d -  %s\n", resp.StatusCode, resp.Text())
		return none, nil
	}
	v, err := resp.JSON()
	if err != nil {
		// requests' JSONDecodeError is a RequestException.
		fmt.Printf("Network or request error fetching PR details: %v\n", err)
		return none, nil
	}
	prData := team.Map(v)
	fmt.Fprintf(&prMessage, "The PR title is: %s\n", team.PyStr(prData["title"]))

	// --- 2. Fetching associated commits (paginated) ---
	if commitsURL := team.Str(prData["commits_url"]); commitsURL != "" {
		fmt.Println("\n--- Associated Commits in this PR: ---")
		for page := 1; ; {
			resp, err := gh.Do(http.MethodGet, commitsURL, team.Params{"per_page": 100, "page": page}, nil, nil)
			if err != nil {
				fmt.Printf("Network or request error fetching PR commits (page %d): %v\n", page, err)
				break
			}
			if resp.RaiseForStatus() != nil {
				fmt.Printf("HTTP Error fetching PR commits (page %d): %d - %s\n", page, resp.StatusCode, resp.Text())
				break
			}
			data, err := resp.JSON()
			if err != nil {
				// requests' JSONDecodeError is a RequestException.
				fmt.Printf("Network or request error fetching PR commits (page %d): %v\n", page, err)
				break
			}
			commits := team.List(data)
			if len(commits) == 0 {
				break
			}
			prMessage.WriteString("The associated commits are:\n")
			for _, c := range commits {
				msg := team.Str(team.Get(c, "commit", "message"))
				first, _, _ := strings.Cut(strings.ReplaceAll(msg, "\r\n", "\n"), "\n")
				if first != "" {
					prMessage.WriteString(first + "\n")
				}
			}
			if link := resp.Header.Get("Link"); link != "" && strings.Contains(link, `rel="next"`) {
				page++
				continue
			}
			break
		}
	} else {
		fmt.Println("Commits URL not found in PR data.")
	}
	return prMessage.String(), nil
}

const systemPrompt = `
You are a helpful assistant to generate reasonable descriptions for pull requests for software engineers.

The descriptions should not be too short (e.g.: less than 3 words), or too long (e.g.: more than 30 words).

The generated description should start with ` + "`chore`, `docs`, `feat`, `fix`, `test`, or `refactor`" + `.
` + "`feat`" + ` stands for a new feature.
` + "`fix`" + ` stands for a bug fix.
` + "`chore`, `docs`, `test`, and `refactor`" + ` stand for improvements.

Some good descriptions are:
1. feat: Added implementation for ` + "`get_eval_case`, `update_eval_case` and `delete_eval_case`" + ` for the local eval sets manager.
2. feat: Provide inject_session_state as public util method.

Some bad descriptions are:
1. fix: This fixes bugs.
2. feat: This is a new feature.

`

const prToolDoc = `Fetches information for a GitHub Pull Request by sending direct HTTP requests.

Args:
    pr_number (int): The number of the Pull Request.

Returns:
    pr_message: A string.`

func main() {
	ctx := context.Background()
	githubToken = team.Getenv("GITHUB_TOKEN", "")
	if githubToken == "" {
		team.Fatalf("ValueError: GITHUB_TOKEN environment variable not set")
	}
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")

	root := team.RootAgent(ctx, llmagent.Config{
		Name:        "github_pr_agent",
		Model:       team.Model(ctx),
		Description: "Generate pull request descriptions for ADK.",
		Instruction: systemPrompt,
		Tools:       []tool.Tool{team.SecuredTool("get_github_pr_info_http", prToolDoc, getGitHubPRInfoHTTP)},
	})

	start := time.Now()
	fmt.Printf("Script start time: %s\n", team.TimeLine(start))
	fmt.Println("------------------------------------")
	run(ctx, root)
	end := time.Now()
	fmt.Println("------------------------------------")
	fmt.Printf("Script end time: %s\n", team.TimeLine(end))
	fmt.Printf("Total script execution time: %.2f seconds\n", end.Sub(start).Seconds())
}

func run(ctx context.Context, root agent.Agent) {
	const (
		appName = "adk_pr_app"
		userID1 = "adk_pr_user"
	)
	runner := team.NewRunner(appName, root)
	session11, err := runner.NewSession(ctx, userID1)
	if err != nil {
		team.Fatalf("%v", err)
	}

	runAgentPrompt := func(sessionID, prompt string) {
		var parts []string
		err := runner.Send(ctx, userID1, sessionID, prompt, func(ev *session.Event) error {
			if ev.Content != nil && len(ev.Content.Parts) > 0 && ev.Content.Parts[0] != nil && ev.Content.Parts[0].Text != "" {
				if ev.Author == root.Name() {
					parts = append(parts, ev.Content.Parts[0].Text)
				}
			}
			return nil
		})
		if err != nil {
			team.Fatalf("%v", err)
		}
		fmt.Printf("<<<< Agent Final Output: %s\n\n", strings.Join(parts, ""))
	}

	// set_current_wallet_addr(the agent's wallet), then the secured tool
	// called directly: it authorizes as the team identity.
	out, err := team.Secured(getGitHubPRInfoHTTP)(team.DirectContext(ctx, team.Identity(ctx)), prArgs{PRNumber: 1422})
	if err != nil {
		team.Fatalf("%v", err)
	}
	prMessage, _ := out.(string)
	if prMessage == "" {
		fmt.Println("Could not fetch the pull request info.")
		return
	}
	runAgentPrompt(session11, "Generate pull request description for "+prMessage)
}
