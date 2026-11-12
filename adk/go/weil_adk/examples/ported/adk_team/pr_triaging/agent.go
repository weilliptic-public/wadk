package main

import (
	"context"
	"fmt"
	"net/http"
	"slices"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/componentowners"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// settings.py
var (
	githubBaseURL     = "https://api.github.com"
	githubGraphQLURL  = githubBaseURL + "/graphql"
	owner, repo       string
	pullRequestNumber string
	prCountToProcess  string
	isInteractive     bool

	github     *team.HTTP // headers
	githubDiff *team.HTTP // diff_headers, no timeout
)

func loadSettings() {
	team.LoadDotEnv("")
	token := team.MustGetenv("GITHUB_TOKEN")
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")
	pullRequestNumber = team.Getenv("PULL_REQUEST_NUMBER", "")
	prCountToProcess = team.Getenv("PR_COUNT_TO_PROCESS", "10")
	isInteractive = team.IsTrue("INTERACTIVE", "1")
	github = &team.HTTP{Headers: team.GitHubHeaders(token), Timeout: 60 * time.Second}
	githubDiff = &team.HTTP{Headers: map[string]string{
		"Authorization": "token " + token,
		"Accept":        "application/vnd.github.v3.diff",
	}}
}

// utils.py

func runGraphQLQuery(query string, variables map[string]any) (map[string]any, error) {
	return github.GraphQL(githubGraphQLURL, query, variables)
}

func getDiff(u string) (string, error) {
	resp, err := githubDiff.Do(http.MethodGet, u, nil, nil, nil)
	if err != nil {
		return "", err
	}
	if err := resp.RaiseForStatus(); err != nil {
		return "", err
	}
	return resp.Text(), nil
}

// isAssignable is is_assignable: whether login can be assigned to an
// issue/PR in this repo. GitHub only allows assignees with repo
// write/triage access and silently drops others from an assignee POST;
// check first so callers can report the skip instead of a silent no-op.
// A failed request raises in Python, which no tool code catches, so it
// aborts the whole run; it exits the program here likewise (a tool error
// would only become a function response the model sees).
func isAssignable(login string) bool {
	resp, err := github.Do(http.MethodGet, fmt.Sprintf("%s/repos/%s/%s/assignees/%s", githubBaseURL, owner, repo, login), nil, nil, nil)
	if err != nil {
		team.Fatalf("%v", err)
	}
	return resp.StatusCode == 204
}

func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "error_message": msg}
}

// agent.py

func approvalInstruction() string {
	if isInteractive {
		return "Only assign when the user approves the action!"
	}
	return "Do not ask for user approval for assigning!" +
		" If you can't tell which component the PR belongs to, leave it" +
		" unassigned."
}

type prNumberArgs struct {
	PRNumber int `json:"pr_number"`
}

const prDetailsQuery = `
    query($owner: String!, $repo: String!, $prNumber: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $prNumber) {
          id
          number
          title
          body
          state
          author {
            login
          }
          labels(last: 10) {
            nodes {
              name
            }
          }
          assignees(first: 10) {
            nodes {
              login
            }
          }
          files(last: 50) {
            nodes {
              path
            }
          }
          comments(last: 50) {
            nodes {
              id
              body
              createdAt
              author {
                login
              }
            }
          }
          commits(last: 50) {
            nodes {
              commit {
                url
                message
              }
            }
          }
          statusCheckRollup {
            state
            contexts(last: 20) {
              nodes {
                ... on StatusContext {
                  context
                  state
                  targetUrl
                }
                ... on CheckRun {
                  name
                  status
                  conclusion
                  detailsUrl
                }
              }
            }
          }
        }
      }
    }
  `

func getPullRequestDetails(_ agent.Context, in prNumberArgs) (map[string]any, error) {
	fmt.Printf("Fetching details for PR #%d from %s/%s\n", in.PRNumber, owner, repo)
	u := fmt.Sprintf("%s/repos/%s/%s/pulls/%d", githubBaseURL, owner, repo, in.PRNumber)

	response, err := runGraphQLQuery(prDetailsQuery, map[string]any{"owner": owner, "repo": repo, "prNumber": in.PRNumber})
	if err != nil {
		return errorResponse(err.Error()), nil
	}
	if errs, ok := response["errors"]; ok {
		return errorResponse(team.Repr(errs)), nil
	}
	pr := team.Map(team.Get(response, "data", "repository", "pullRequest"))
	if len(pr) == 0 {
		return errorResponse(fmt.Sprintf("Pull Request #%d not found.", in.PRNumber)), nil
	}

	// Filter out main merge commits.
	if commits := team.Map(pr["commits"]); commits != nil {
		if nodes := team.List(commits["nodes"]); len(nodes) > 0 {
			filtered := []any{}
			for _, node := range nodes {
				if !strings.HasPrefix(team.Str(team.Get(node, "commit", "message")), "Merge branch 'main' into") {
					filtered = append(filtered, node)
				}
			}
			commits["nodes"] = filtered
		}
	}

	// Get diff of the PR and truncate it to avoid exceeding the maximum tokens.
	diff, err := getDiff(u)
	if err != nil {
		return errorResponse(err.Error()), nil
	}
	if r := []rune(diff); len(r) > 10000 {
		diff = string(r[:10000])
	}
	pr["diff"] = diff
	return map[string]any{"status": "success", "pull_request": pr}, nil
}

type assignArgs struct {
	PRNumber  int    `json:"pr_number"`
	Component string `json:"component"`
}

func assignOwnerToPR(_ agent.Context, in assignArgs) (map[string]any, error) {
	owners := componentowners.LabelToOwner[in.Component]
	if len(owners) == 0 {
		return errorResponse(fmt.Sprintf("Error: no owner mapped for component '%s'.", in.Component)), nil
	}
	start := ((in.PRNumber % len(owners)) + len(owners)) % len(owners)
	var assignee string
	for _, o := range append(slices.Clone(owners[start:]), owners[:start]...) {
		if isAssignable(o) {
			assignee = o
			break
		}
	}
	if assignee == "" {
		return map[string]any{
			"status": "skipped",
			"reason": fmt.Sprintf("none of %s is assignable (needs repo access)", team.Repr(owners)),
			"owners": owners,
		}, nil
	}
	fmt.Printf("Attempting to assign owner '%s' to PR #%d\n", assignee, in.PRNumber)

	// Pull Request is a special issue in GitHub, so we can use the issue url.
	assigneeURL := fmt.Sprintf("%s/repos/%s/%s/issues/%d/assignees", githubBaseURL, owner, repo, in.PRNumber)
	response, err := github.PostJSON(assigneeURL, map[string]any{"assignees": []string{assignee}})
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "assigned_owner": assignee, "response": response}, nil
}

type prCountArgs struct {
	PRCount int `json:"pr_count"`
}

func listUnassignedPullRequests(_ agent.Context, in prCountArgs) (map[string]any, error) {
	u := githubBaseURL + "/search/issues"
	params := team.Params{
		"q":        fmt.Sprintf("repo:%s/%s is:open is:pr no:assignee", owner, repo),
		"sort":     "updated",
		"order":    "desc",
		"per_page": 100,
		"page":     1,
	}
	response, err := github.GetJSON(u, params)
	if err != nil {
		return errorResponse(fmt.Sprintf("Error: %v", err)), nil
	}

	unassigned := []any{}
	for _, item := range team.List(team.Get(response, "items")) {
		pr := team.Map(item)
		google := false
		for _, l := range team.List(pr["labels"]) {
			if team.Str(team.Get(l, "name")) == "google-contributor" {
				google = true
			}
		}
		if google {
			continue
		}
		unassigned = append(unassigned, map[string]any{"number": pr["number"], "title": pr["title"]})
		if len(unassigned) >= in.PRCount {
			break
		}
	}
	return map[string]any{"status": "success", "pull_requests": unassigned}, nil
}

func newRootAgent(ctx context.Context) agent.Agent {
	return team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "adk_pr_triaging_assistant",
		Description: "Assign component owners to ADK pull requests.",
		Instruction: instruction(),
		Tools: []tool.Tool{
			team.SecuredTool("list_unassigned_pull_requests", `List open pull requests that have nobody assigned.

Skips pull requests labeled google-contributor, which are shepherded by their
own author.

Args:
  pr_count: number of pull requests to return

Returns:
  The status of this request, with a list of pull requests when successful.`, listUnassignedPullRequests),
			team.SecuredTool("get_pull_request_details", `Get the details of the specified pull request.

Args:
  pr_number: number of the GitHub pull request.

Returns:
  The status of this request, with the details when successful.`, getPullRequestDetails),
			team.SecuredTool("assign_owner_to_pr", `Assign one of the component's owners (the shepherd) to a PR.

The owners are looked up from `+"`LABEL_TO_OWNER`"+` so the contributor can see who
is shepherding their PR. A component with several owners starts at the owner
picked by the PR number, so they take turns. GitHub only allows assigning
users with repo write/triage access, so a non-assignable owner is passed over,
and a PR none of the owners can take is reported as skipped rather than
silently dropped.

Args:
  pr_number: the number of the GitHub pull request
  component: the component the PR belongs to

Returns:
  The status of this request, with the assigned owner when successful.`, assignOwnerToPR),
		},
	})
}
