// Package docs is the Go port of adk_team/adk_documentation: its settings,
// utils and the GitHub / local git tools shared by the release analyzer and
// the docs updater.
package docs

import (
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// Settings, as adk_documentation/settings.py (read on import, after
// load_dotenv(override=True)).
const GitHubBaseURL = "https://api.github.com"

var (
	GitHubToken       string
	DocOwner          string
	CodeOwner         string
	DocRepo           string
	CodeRepo          string
	LocalReposDirPath string
	IsInteractive     bool

	headers map[string]string
	client  *team.HTTP
)

func init() {
	team.LoadDotEnv("")
	GitHubToken = team.MustGetenv("GITHUB_TOKEN")
	DocOwner = team.Getenv("DOC_OWNER", "google")
	CodeOwner = team.Getenv("CODE_OWNER", "google")
	DocRepo = team.Getenv("DOC_REPO", "adk-docs")
	CodeRepo = team.Getenv("CODE_REPO", "adk-python")
	LocalReposDirPath = team.Getenv("LOCAL_REPOS_DIR_PATH", "/tmp")
	IsInteractive = team.IsTrue("INTERACTIVE", "1")

	headers = team.GitHubHeaders(GitHubToken)
	client = &team.HTTP{Headers: headers, Timeout: 60 * time.Second}
}

// ErrorResponse is utils.error_response.
func ErrorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "error_message": msg}
}

// GetRequest is utils.get_request with the default headers.
func GetRequest(url string, params team.Params) (any, error) {
	return client.GetJSON(url, params)
}

// GetPaginatedRequest is utils.get_paginated_request: GETs url and follows
// the Link header's "next" until it runs out, concatenating the arrays.
func GetPaginatedRequest(url string) ([]any, error) {
	var results []any
	for url != "" {
		resp, err := client.Do("GET", url, nil, nil, nil)
		if err != nil {
			return nil, err
		}
		if err := resp.RaiseForStatus(); err != nil {
			return nil, err
		}
		v, err := resp.JSON()
		if err != nil {
			return nil, err
		}
		results = append(results, team.List(v)...)
		url = resp.Links()["next"]
	}
	return results, nil
}

// PostRequest is utils.post_request.
func PostRequest(url string, payload any) (any, error) { return client.PostJSON(url, payload) }

// PatchRequest is utils.patch_request.
func PatchRequest(url string, payload any) (any, error) { return client.PatchJSON(url, payload) }

// Suggestion is one (number, text) pair of ParseSuggestions.
type Suggestion struct {
	Num  int
	Text string
}

var suggestionPatterns = []struct{ start, match *regexp.Regexp }{
	// Format A: "### 1. Title" (markdown header with number)
	{regexp.MustCompile(`(?m)^###\s+\d+\.`), regexp.MustCompile(`^###\s+(\d+)\.`)},
	// Format B: "1. **Title**" (numbered list with bold)
	{regexp.MustCompile(`(?m)^\d+\.\s+\*\*`), regexp.MustCompile(`^(\d+)\.\s+\*\*`)},
}

// ParseSuggestions is utils.parse_suggestions: the numbered suggestions of
// an issue body, as "### 1. Title" headers or "1. **Title**" items.
func ParseSuggestions(body string) []Suggestion {
	for _, p := range suggestionPatterns {
		// re.split on a lookahead: cut before every match start.
		var parts []string
		last := 0
		for _, loc := range p.start.FindAllStringIndex(body, -1) {
			if loc[0] == 0 {
				continue
			}
			parts = append(parts, body[last:loc[0]])
			last = loc[0]
		}
		parts = append(parts, body[last:])

		var out []Suggestion
		for _, part := range parts {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			if m := p.match.FindStringSubmatch(part); m != nil {
				n, _ := strconv.Atoi(m[1])
				out = append(out, Suggestion{Num: n, Text: part})
			}
		}
		if len(out) > 0 {
			return out
		}
	}
	return nil
}
