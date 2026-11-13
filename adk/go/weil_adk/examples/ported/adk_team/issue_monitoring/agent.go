package main

import (
	"context"
	_ "embed"
	"fmt"
	"path/filepath"
	"runtime"
	"slices"
	"strconv"
	"strings"
	"sync/atomic"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// settings.py
var (
	githubBaseURL      = "https://api.github.com"
	owner, repo        string
	spamLabelName      string
	concurrencyLimit   int
	botName            string
	botAlertSignature  string
	sleepBetweenChunks float64
	initialFullScan    bool
)

func loadSettings() {
	_, file, _, _ := runtime.Caller(0)
	team.LoadDotEnv(filepath.Join(filepath.Dir(file), ".env"))
	token := team.MustGetenv("GITHUB_TOKEN")
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")
	spamLabelName = team.Getenv("SPAM_LABEL_NAME", "spam")
	var err error
	if concurrencyLimit, err = strconv.Atoi(team.Getenv("CONCURRENCY_LIMIT", "3")); err != nil {
		team.Fatalf("ValueError: invalid literal for int() with base 10: %s", team.Getenv("CONCURRENCY_LIMIT", ""))
	}
	botName = team.Getenv("BOT_NAME", "adk-bot")
	botAlertSignature = team.Getenv("BOT_ALERT_SIGNATURE", "🚨 **Automated Spam Detection Alert** 🚨")
	if sleepBetweenChunks, err = strconv.ParseFloat(team.Getenv("SLEEP_BETWEEN_CHUNKS", "1.5"), 64); err != nil {
		team.Fatalf("ValueError: could not convert string to float: %s", team.Getenv("SLEEP_BETWEEN_CHUNKS", ""))
	}
	initialFullScan = strings.ToLower(team.Getenv("INITIAL_FULL_SCAN", "false")) == "true"

	github = &team.HTTP{
		Headers: team.GitHubHeaders(token),
		Timeout: 60 * time.Second,
		Retry: &team.Retry{
			Total:           6,
			BackoffFactor:   2,
			StatusForcelist: []int{429, 500, 502, 503, 504},
			AllowedMethods:  []string{"GET", "DELETE"},
		},
		OnRequest: func() { apiCallCount.Add(1) },
	}
}

// utils.py

var (
	github       *team.HTTP
	apiCallCount atomic.Int64
	log          = team.Log
)

func getAPICallCount() int64 { return apiCallCount.Load() }
func resetAPICallCount()     { apiCallCount.Store(0) }

func getRequest(u string, params team.Params) (any, error) { return github.GetJSON(u, params) }
func postRequest(u string, payload any) (any, error)       { return github.PostJSON(u, payload) }

func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "message": msg}
}

func getRepositoryMaintainers(owner, repo string) ([]string, error) {
	data, err := getRequest(fmt.Sprintf("https://api.github.com/repos/%s/%s/collaborators", owner, repo),
		team.Params{"permission": "push"})
	if err != nil {
		return nil, err
	}
	var logins []string
	for _, u := range team.List(data) {
		logins = append(logins, team.Str(team.Get(u, "login")))
	}
	return logins, nil
}

func getIssueDetails(owner, repo string, number int) (map[string]any, error) {
	v, err := getRequest(fmt.Sprintf("https://api.github.com/repos/%s/%s/issues/%d", owner, repo, number), nil)
	return team.Map(v), err
}

func getIssueComments(owner, repo string, number int) ([]any, error) {
	u := fmt.Sprintf("https://api.github.com/repos/%s/%s/issues/%d/comments", owner, repo, number)
	all := []any{}
	for page := 1; ; page++ {
		data, err := getRequest(u, team.Params{"per_page": 100, "page": page})
		if err != nil {
			return nil, err
		}
		items := team.List(data)
		if len(items) == 0 {
			break
		}
		all = append(all, items...)
		if len(items) < 100 {
			break
		}
	}
	return all, nil
}

func getTargetIssues(owner, repo string) ([]int, error) {
	u := fmt.Sprintf("https://api.github.com/repos/%s/%s/issues", owner, repo)
	params := team.Params{"state": "open", "per_page": 100}
	if initialFullScan {
		log.Infof("INITIAL_FULL_SCAN is True. Fetching ALL open issues...")
	} else {
		yesterday := time.Now().UTC().Add(-24 * time.Hour).Format("2006-01-02T15:04:05Z")
		params["since"] = yesterday
		log.Infof("Daily mode: Fetching issues updated since %s...", yesterday)
	}

	var numbers []int
	for page := 1; ; page++ {
		params["page"] = page
		data, err := getRequest(u, params)
		if err != nil {
			log.Errorf("Failed to fetch issues on page %d: %v", page, err)
			return nil, err
		}
		items := team.List(data)
		if len(items) == 0 {
			break
		}
		for _, item := range items {
			m := team.Map(item)
			if _, isPR := m["pull_request"]; isPR {
				continue
			}
			var labels []string
			for _, l := range team.List(m["labels"]) {
				labels = append(labels, team.Str(team.Get(l, "name")))
			}
			if !slices.Contains(labels, spamLabelName) {
				numbers = append(numbers, team.Int(m["number"]))
			} else {
				log.Debugf("Skipping #%d - already marked as spam.", team.Int(m["number"]))
			}
		}
		if len(items) < 100 {
			break
		}
	}
	return numbers, nil
}

// agent.py

//go:embed PROMPT_INSTRUCTION.txt
var promptTemplate string

type flagArgs struct {
	ItemNumber      int    `json:"item_number"`
	DetectionReason string `json:"detection_reason"`
	IsOriginalIssue bool   `json:"is_original_issue,omitempty"`
}

func flagIssueAsSpam(_ agent.Context, in flagArgs) (map[string]any, error) {
	log.Infof("Flagging spam on #%d (is_original_issue=%s). Reason: %s",
		in.ItemNumber, pyBool(in.IsOriginalIssue), in.DetectionReason)

	labelURL := fmt.Sprintf("%s/repos/%s/%s/issues/%d/labels", githubBaseURL, owner, repo, in.ItemNumber)
	commentURL := fmt.Sprintf("%s/repos/%s/%s/issues/%d/comments", githubBaseURL, owner, repo, in.ItemNumber)

	safeReason := strings.ReplaceAll(in.DetectionReason, "```", "'''")
	scopeNote := "suspected spam comment was detected in this thread."
	if in.IsOriginalIssue {
		scopeNote = "suspected spam issue description was detected."
	}
	alertBody := botAlertSignature + "\n" +
		"@maintainers, a " + scopeNote + "\n\n" +
		"**Reason:**\n" +
		"```text\n" + safeReason + "\n```"

	fail := func(err error) (map[string]any, error) {
		return errorResponse(fmt.Sprintf("Error flagging issue: %v", err)), nil
	}
	issue, err := getIssueDetails(owner, repo, in.ItemNumber)
	if err != nil {
		return fail(err)
	}
	comments, err := getIssueComments(owner, repo, in.ItemNumber)
	if err != nil {
		return fail(err)
	}
	isLabeled := false
	for _, l := range team.List(issue["labels"]) {
		if strings.ToLower(team.Str(team.Get(l, "name"))) == strings.ToLower(spamLabelName) {
			isLabeled = true
		}
	}
	isCommented := false
	for _, c := range comments {
		if strings.Contains(team.Str(team.Get(c, "body")), botAlertSignature) {
			isCommented = true
		}
	}

	if !isCommented {
		if _, err := postRequest(commentURL, map[string]any{"body": alertBody}); err != nil {
			return fail(err)
		}
		log.Infof("Successfully posted spam alert comment to #%d.", in.ItemNumber)
	}
	if in.IsOriginalIssue && !isLabeled {
		if _, err := postRequest(labelURL, map[string]any{"labels": []string{spamLabelName}}); err != nil {
			return fail(err)
		}
		log.Infof("Successfully added '%s' label to #%d.", spamLabelName, in.ItemNumber)
	} else if !in.IsOriginalIssue {
		log.Infof("Spam detected only in comment on #%d; omitted '%s' issue label.", in.ItemNumber, spamLabelName)
	}
	return map[string]any{"status": "success", "message": "Maintainers alerted successfully."}, nil
}

func pyBool(b bool) string {
	if b {
		return "True"
	}
	return "False"
}

func newRootAgent(ctx context.Context) agent.Agent {
	return team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "spam_auditor_agent",
		Description: "Audits issue comments for spam.",
		Instruction: strings.NewReplacer("{OWNER}", owner, "{REPO}", repo).Replace(promptTemplate),
		Tools: []tool.Tool{
			team.SecuredTool("flag_issue_as_spam", `Flags spam detected in an issue thread.

Leaves an alert comment for maintainers, and adds the spam label ONLY if the
original issue description itself is spam. If only a reply comment in the
thread is spam, alerts maintainers with a comment without applying the spam
label to the entire issue.

Args:
    item_number (int): The GitHub issue number.
    detection_reason (str): The explanation of what the spam is.
    is_original_issue (bool): Set to True if and only if the original issue
      description itself is spam. Defaults to False when only a reply comment
      contains spam.`, flagIssueAsSpam),
		},
	})
}
