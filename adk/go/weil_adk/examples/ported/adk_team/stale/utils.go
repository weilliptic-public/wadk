package main

import (
	"fmt"
	"math"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync/atomic"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// --- Settings (settings.py) ---

const (
	githubBaseURL             = "https://api.github.com"
	staleLabelName            = "stale"
	requestClarificationLabel = "request clarification"
)

var (
	githubToken                   string
	owner                         string
	repo                          string
	staleHoursThreshold           float64
	closeHoursAfterStaleThreshold float64
	concurrencyLimit              int
	graphqlCommentLimit           int
	graphqlEditLimit              int
	graphqlTimelineLimit          int
	sleepBetweenChunks            float64
)

func loadSettings() {
	team.LoadDotEnv("")
	githubToken = team.MustGetenv("GITHUB_TOKEN")
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")
	staleHoursThreshold = envFloat("STALE_HOURS_THRESHOLD", 168)
	closeHoursAfterStaleThreshold = envFloat("CLOSE_HOURS_AFTER_STALE_THRESHOLD", 168)
	concurrencyLimit = envInt("CONCURRENCY_LIMIT", 3)
	graphqlCommentLimit = envInt("GRAPHQL_COMMENT_LIMIT", 30)
	graphqlEditLimit = envInt("GRAPHQL_EDIT_LIMIT", 10)
	graphqlTimelineLimit = envInt("GRAPHQL_TIMELINE_LIMIT", 20)
	sleepBetweenChunks = envFloat("SLEEP_BETWEEN_CHUNKS", 1.5)

	httpSession = &team.HTTP{
		Headers: team.GitHubHeaders(githubToken),
		Timeout: 60 * time.Second,
		Retry: &team.Retry{
			Total:           6,
			BackoffFactor:   2,
			StatusForcelist: []int{429, 500, 502, 503, 504},
			AllowedMethods:  []string{"HEAD", "GET", "POST", "PUT", "DELETE", "OPTIONS", "TRACE", "PATCH"},
		},
		OnRequest: func() { apiCallCount.Add(1) },
	}
}

func envFloat(key string, def float64) float64 {
	s, ok := os.LookupEnv(key)
	if !ok {
		return def
	}
	f, err := strconv.ParseFloat(strings.TrimSpace(s), 64)
	if err != nil {
		team.Fatalf("ValueError: could not convert string to float: %q", s)
	}
	return f
}

func envInt(key string, def int) int {
	s, ok := os.LookupEnv(key)
	if !ok {
		return def
	}
	n, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil {
		team.Fatalf("ValueError: invalid literal for int() with base 10: %q", s)
	}
	return n
}

// pyFloat formats f as Python's str(float) does: 7.0, 0.5, 1.5.
func pyFloat(f float64) string {
	if f == math.Trunc(f) && math.Abs(f) < 1e16 {
		return strconv.FormatFloat(f, 'f', 1, 64)
	}
	return strconv.FormatFloat(f, 'g', -1, 64)
}

// --- API call counter (utils.py) ---

var apiCallCount atomic.Int64

func getAPICallCount() int { return int(apiCallCount.Load()) }
func resetAPICallCount()   { apiCallCount.Store(0) }
func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "message": msg}
}

// httpSession is the shared requests.Session with exponential-backoff retries.
var httpSession *team.HTTP

func send(method, url string, params team.Params, payload any) (any, error) {
	resp, err := httpSession.Do(method, url, params, payload, nil)
	if err == nil {
		err = resp.RaiseForStatus()
	}
	if err != nil {
		team.Log.Errorf("%s request failed for %s: %v", method, url, err)
		return nil, err
	}
	if method == http.MethodDelete && resp.StatusCode == 204 {
		return map[string]any{"status": "success", "message": "Deletion successful."}, nil
	}
	v, err := resp.JSON()
	if err != nil {
		team.Log.Errorf("%s request failed for %s: %v", method, url, err)
	}
	return v, err
}

func getRequest(url string, params team.Params) (any, error) {
	return send(http.MethodGet, url, params, nil)
}
func postRequest(url string, payload any) (any, error) {
	return send(http.MethodPost, url, nil, payload)
}
func patchRequest(url string, payload any) (any, error) {
	return send(http.MethodPatch, url, nil, payload)
}
func deleteRequest(url string) (any, error) { return send(http.MethodDelete, url, nil, nil) }

// getOldOpenIssueNumbers finds open issues created more than daysOld days
// ago with the Search API's created:<DATE filter.
func getOldOpenIssueNumbers(owner, repo string, daysOld float64) ([]int, error) {
	cutoff := time.Now().UTC().Add(-time.Duration(daysOld * 24 * float64(time.Hour)))
	cutoffStr := cutoff.Format("2006-01-02T15:04:05Z")
	query := fmt.Sprintf("repo:%s/%s is:issue state:open created:<%s", owner, repo, cutoffStr)

	team.Log.Infof("Searching for issues in '%s/%s' created before %s...", owner, repo, cutoffStr)

	var issueNumbers []int
	page := 1
	url := "https://api.github.com/search/issues"
	for {
		data, err := getRequest(url, team.Params{"q": query, "per_page": 100, "page": page})
		if err != nil {
			team.Log.Errorf("GitHub search failed on page %d: %v", page, err)
			return nil, err
		}
		items := team.List(team.Get(data, "items"))
		if len(items) == 0 {
			break
		}
		for _, item := range items {
			if _, isPR := team.Map(item)["pull_request"]; !isPR {
				issueNumbers = append(issueNumbers, team.Int(team.Get(item, "number")))
			}
		}
		if len(items) < 100 {
			break
		}
		page++
	}
	team.Log.Infof("Found %d stale issues.", len(issueNumbers))
	return issueNumbers, nil
}
