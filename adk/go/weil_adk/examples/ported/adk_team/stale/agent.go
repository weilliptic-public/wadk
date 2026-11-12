package main

import (
	"context"
	_ "embed"
	"errors"
	"fmt"
	"math"
	"slices"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// Used to detect if the bot has already posted an alert to avoid spamming.
const (
	botAlertSignature = "**Notification:** The author has updated the issue description"
	botName           = "adk-bot"
)

//go:embed PROMPT_INSTRUCTION.txt
var promptTemplate string

// requestError marks failures Python raises as requests' RequestException.
type requestError struct{ msg string }

func (e *requestError) Error() string { return e.msg }

// --- Global Cache ---
var (
	maintainersMu    sync.Mutex
	maintainersCache []string
)

// getCachedMaintainers fetches the repository's maintainers once. Failure
// aborts processing, so the bot never runs with incorrect permissions.
func getCachedMaintainers() ([]string, error) {
	maintainersMu.Lock()
	defer maintainersMu.Unlock()
	if maintainersCache != nil {
		return maintainersCache, nil
	}
	team.Log.Infof("Initializing Maintainers Cache...")

	err := func() error {
		url := fmt.Sprintf("%s/repos/%s/%s/collaborators", githubBaseURL, owner, repo)
		data, err := getRequest(url, team.Params{"permission": "push"})
		if err != nil {
			return err
		}
		list, ok := data.([]any)
		if !ok {
			team.Log.Errorf("Invalid API response format: Expected list, got %T", data)
			return fmt.Errorf("GitHub API returned non-list data: %s", team.Repr(data))
		}
		cache := []string{}
		for _, u := range list {
			if login, ok := team.Map(u)["login"]; ok {
				cache = append(cache, team.Str(login))
			}
		}
		maintainersCache = cache
		team.Log.Infof("Cached %d maintainers.", len(cache))
		return nil
	}()
	if err != nil {
		team.Log.Criticalf("FATAL: Failed to verify repository maintainers. Error: %v", err)
		return nil, errors.New("Maintainer verification failed. processing aborted.")
	}
	return maintainersCache, nil
}

// fetchGraphQLData fetches the issue's comments, edits and timeline events.
func fetchGraphQLData(itemNumber int) (map[string]any, error) {
	query := `
    query($owner: String!, $name: String!, $number: Int!, $commentLimit: Int!, $timelineLimit: Int!, $editLimit: Int!) {
      repository(owner: $owner, name: $name) {
        issue(number: $number) {
          author { login }
          createdAt
          labels(first: 20) { nodes { name } }

          comments(last: $commentLimit) {
            nodes {
              author { login }
              body
              createdAt
              lastEditedAt
            }
          }

          userContentEdits(last: $editLimit) {
            nodes {
              editor { login }
              editedAt
            }
          }

          timelineItems(itemTypes: [LABELED_EVENT, RENAMED_TITLE_EVENT, REOPENED_EVENT], last: $timelineLimit) {
            nodes {
              __typename
              ... on LabeledEvent {
                createdAt
                actor { login }
                label { name }
              }
              ... on RenamedTitleEvent {
                createdAt
                actor { login }
              }
              ... on ReopenedEvent {
                createdAt
                actor { login }
              }
            }
          }
        }
      }
    }
    `
	variables := map[string]any{
		"owner":         owner,
		"name":          repo,
		"number":        itemNumber,
		"commentLimit":  graphqlCommentLimit,
		"editLimit":     graphqlEditLimit,
		"timelineLimit": graphqlTimelineLimit,
	}
	response, err := postRequest(githubBaseURL+"/graphql", map[string]any{"query": query, "variables": variables})
	if err != nil {
		return nil, &requestError{err.Error()}
	}
	if errs, ok := team.Map(response)["errors"]; ok {
		return nil, &requestError{fmt.Sprintf("GraphQL Error: %s", team.Str(team.Get(team.List(errs)[0], "message")))}
	}
	data := team.Map(team.Get(response, "data", "repository", "issue"))
	if len(data) == 0 {
		return nil, &requestError{fmt.Sprintf("Issue #%d not found.", itemNumber)}
	}
	return data, nil
}

type historyEvent struct {
	Type  string
	Actor string
	Time  time.Time
	Data  any // comment body for "commented", else nil
}

// isoparse is dateutil.parser.isoparse for GitHub's ISO 8601 timestamps.
func isoparse(v any) (time.Time, error) {
	s := team.Str(v)
	t, err := time.Parse(time.RFC3339Nano, s)
	if err != nil {
		return time.Time{}, fmt.Errorf("ISO string too short: %q", s)
	}
	return t, nil
}

func isHuman(actor string) bool {
	return actor != "" && !strings.HasSuffix(actor, "[bot]") && actor != botName
}

// buildHistoryTimeline turns the GraphQL data into a chronologically sorted
// history, the stale label's application times and the last bot alert time.
func buildHistoryTimeline(data map[string]any) ([]historyEvent, []time.Time, *time.Time, error) {
	issueAuthor := team.Str(team.Get(data, "author", "login"))
	var history []historyEvent
	var labelEvents []time.Time
	var lastBotAlertTime *time.Time

	// 1. Baseline: Issue Creation
	created, err := isoparse(data["createdAt"])
	if err != nil {
		return nil, nil, nil, err
	}
	history = append(history, historyEvent{Type: "created", Actor: issueAuthor, Time: created})

	// 2. Process Comments
	for _, c := range team.List(team.Get(data, "comments", "nodes")) {
		if c == nil {
			continue
		}
		actor := team.Str(team.Get(c, "author", "login"))
		cBody := team.Str(team.Get(c, "body"))
		cTime, err := isoparse(team.Get(c, "createdAt"))
		if err != nil {
			return nil, nil, nil, err
		}
		// Track bot alerts for spam prevention
		if strings.Contains(cBody, botAlertSignature) {
			if lastBotAlertTime == nil || cTime.After(*lastBotAlertTime) {
				t := cTime
				lastBotAlertTime = &t
			}
			continue
		}
		if isHuman(actor) {
			// Use edit time if available, otherwise creation time
			actual := cTime
			if e := team.Str(team.Get(c, "lastEditedAt")); e != "" {
				if actual, err = isoparse(e); err != nil {
					return nil, nil, nil, err
				}
			}
			history = append(history, historyEvent{Type: "commented", Actor: actor, Time: actual, Data: cBody})
		}
	}

	// 3. Process Body Edits ("Ghost Edits")
	for _, e := range team.List(team.Get(data, "userContentEdits", "nodes")) {
		if e == nil {
			continue
		}
		actor := team.Str(team.Get(e, "editor", "login"))
		if isHuman(actor) {
			t, err := isoparse(team.Get(e, "editedAt"))
			if err != nil {
				return nil, nil, nil, err
			}
			history = append(history, historyEvent{Type: "edited_description", Actor: actor, Time: t})
		}
	}

	// 4. Process Timeline Events
	for _, t := range team.List(team.Get(data, "timelineItems", "nodes")) {
		if t == nil {
			continue
		}
		etype := team.Str(team.Get(t, "__typename"))
		actor := team.Str(team.Get(t, "actor", "login"))
		timeVal, err := isoparse(team.Get(t, "createdAt"))
		if err != nil {
			return nil, nil, nil, err
		}
		if etype == "LabeledEvent" {
			if team.Str(team.Get(t, "label", "name")) == staleLabelName {
				labelEvents = append(labelEvents, timeVal)
			}
			continue
		}
		if isHuman(actor) {
			pretty := "reopened"
			if etype == "RenamedTitleEvent" {
				pretty = "renamed_title"
			}
			history = append(history, historyEvent{Type: pretty, Actor: actor, Time: timeVal})
		}
	}

	// Sort chronologically (stable, as Python's sort)
	sort.SliceStable(history, func(i, j int) bool { return history[i].Time.Before(history[j].Time) })
	return history, labelEvents, lastBotAlertTime, nil
}

type issueState struct {
	lastActionRole   string
	lastActivityTime time.Time
	lastActionType   string
	lastCommentText  any
	lastActorName    string
}

// replayHistoryToFindState replays the history to find the last actor and
// their role.
func replayHistoryToFindState(history []historyEvent, maintainers []string, issueAuthor string) issueState {
	s := issueState{
		lastActionRole:   "author",
		lastActivityTime: history[0].Time,
		lastActionType:   "created",
		lastActorName:    issueAuthor,
	}
	for _, event := range history {
		role := "other_user"
		if event.Actor == issueAuthor {
			role = "author"
		} else if slices.Contains(maintainers, event.Actor) {
			role = "maintainer"
		}
		s.lastActionRole = role
		s.lastActivityTime = event.Time
		s.lastActionType = event.Type
		s.lastActorName = event.Actor
		// Only store text if it was a comment
		if event.Type == "commented" {
			s.lastCommentText = event.Data
		} else {
			s.lastCommentText = nil
		}
	}
	return s
}

func nullable(s string) any {
	if s == "" {
		return nil
	}
	return s
}

type itemArgs struct {
	ItemNumber int `json:"item_number"`
}

type labelArgs struct {
	ItemNumber int    `json:"item_number"`
	LabelName  string `json:"label_name"`
}

func getIssueState(_ agent.Context, in itemArgs) (map[string]any, error) {
	itemNumber := in.ItemNumber
	result, err := func() (map[string]any, error) {
		maintainers, err := getCachedMaintainers()
		if err != nil {
			return nil, err
		}
		// 1. Fetch
		rawData, err := fetchGraphQLData(itemNumber)
		if err != nil {
			return nil, err
		}
		issueAuthor := team.Str(team.Get(rawData, "author", "login"))
		labelsList := []string{}
		for _, l := range team.List(team.Get(rawData, "labels", "nodes")) {
			labelsList = append(labelsList, team.Str(team.Get(l, "name")))
		}

		// 2. Parse & Sort
		history, labelEvents, lastBotAlertTime, err := buildHistoryTimeline(rawData)
		if err != nil {
			return nil, err
		}

		// 3. Analyze (Replay)
		state := replayHistoryToFindState(history, maintainers, issueAuthor)

		// 4. Final Calculations & Alert Logic
		now := time.Now().UTC()
		daysSinceActivity := now.Sub(state.lastActivityTime).Seconds() / 86400

		// Stale Checks
		isStale := slices.Contains(labelsList, staleLabelName)
		daysSinceStaleLabel := 0.0
		if isStale && len(labelEvents) > 0 {
			latest := slices.MaxFunc(labelEvents, func(a, b time.Time) int { return a.Compare(b) })
			daysSinceStaleLabel = now.Sub(latest).Seconds() / 86400
		}

		// Silent Edit Alert Logic
		maintainerAlertNeeded := false
		if (state.lastActionRole == "author" || state.lastActionRole == "other_user") &&
			state.lastActionType == "edited_description" {
			if lastBotAlertTime != nil && lastBotAlertTime.After(state.lastActivityTime) {
				team.Log.Infof("#%d: Silent edit detected, but Bot already alerted. No spam.", itemNumber)
			} else {
				maintainerAlertNeeded = true
				team.Log.Infof("#%d: Silent edit detected. Alert needed.", itemNumber)
			}
		}

		team.Log.Debugf("#%d VERDICT: Role=%s, Idle=%.2fd", itemNumber, state.lastActionRole, daysSinceActivity)

		return map[string]any{
			"status":                  "success",
			"last_action_role":        state.lastActionRole,
			"last_action_type":        state.lastActionType,
			"last_actor_name":         nullable(state.lastActorName),
			"maintainer_alert_needed": maintainerAlertNeeded,
			"is_stale":                isStale,
			"days_since_activity":     daysSinceActivity,
			"days_since_stale_label":  daysSinceStaleLabel,
			"last_comment_text":       state.lastCommentText,
			"current_labels":          labelsList,
			"stale_threshold_days":    staleHoursThreshold / 24,
			"close_threshold_days":    closeHoursAfterStaleThreshold / 24,
			"maintainers":             maintainers,
			"issue_author":            nullable(issueAuthor),
		}, nil
	}()
	if err != nil {
		var re *requestError
		if errors.As(err, &re) || isNetworkError(err) {
			return errorResponse(fmt.Sprintf("Network Error: %v", err)), nil
		}
		team.Log.Errorf("Unexpected error analyzing #%d: %v", itemNumber, err)
		return errorResponse(fmt.Sprintf("Analysis Error: %v", err)), nil
	}
	return result, nil
}

// isNetworkError reports whether err came from the HTTP layer (requests'
// RequestException) rather than from the analysis itself.
func isNetworkError(err error) bool {
	var he *team.HTTPError
	return errors.As(err, &he)
}

// formatDays formats hours as days: 168.0 -> "7", 12.0 -> "0.5".
func formatDays(hours float64) string {
	days := hours / 24
	if math.Mod(days, 1) != 0 {
		return strconv.FormatFloat(days, 'f', 1, 64)
	}
	return strconv.Itoa(int(days))
}

func addLabelToIssue(_ agent.Context, in labelArgs) (map[string]any, error) {
	team.Log.Debugf("Adding label '%s' to issue #%d.", in.LabelName, in.ItemNumber)
	url := fmt.Sprintf("%s/repos/%s/%s/issues/%d/labels", githubBaseURL, owner, repo, in.ItemNumber)
	if _, err := postRequest(url, []string{in.LabelName}); err != nil {
		return errorResponse(fmt.Sprintf("Error adding label: %v", err)), nil
	}
	return map[string]any{"status": "success"}, nil
}

func removeLabelFromIssue(_ agent.Context, in labelArgs) (map[string]any, error) {
	team.Log.Debugf("Removing label '%s' from issue #%d.", in.LabelName, in.ItemNumber)
	url := fmt.Sprintf("%s/repos/%s/%s/issues/%d/labels/%s", githubBaseURL, owner, repo, in.ItemNumber, in.LabelName)
	if _, err := deleteRequest(url); err != nil {
		return errorResponse(fmt.Sprintf("Error removing label: %v", err)), nil
	}
	return map[string]any{"status": "success"}, nil
}

func addStaleLabelAndComment(_ agent.Context, in itemArgs) (map[string]any, error) {
	staleDays := formatDays(staleHoursThreshold)
	closeDays := formatDays(closeHoursAfterStaleThreshold)
	comment := "This issue has been automatically marked as stale because it has not" +
		fmt.Sprintf(" had recent activity for %s days after a maintainer", staleDays) +
		" requested clarification. It will be closed if no further activity" +
		fmt.Sprintf(" occurs within %s days.", closeDays)
	base := fmt.Sprintf("%s/repos/%s/%s/issues/%d", githubBaseURL, owner, repo, in.ItemNumber)
	if _, err := postRequest(base+"/comments", map[string]any{"body": comment}); err != nil {
		return errorResponse(fmt.Sprintf("Error marking issue as stale: %v", err)), nil
	}
	if _, err := postRequest(base+"/labels", []string{staleLabelName}); err != nil {
		return errorResponse(fmt.Sprintf("Error marking issue as stale: %v", err)), nil
	}
	return map[string]any{"status": "success"}, nil
}

func alertMaintainerOfEdit(_ agent.Context, in itemArgs) (map[string]any, error) {
	// Uses the constant signature so get_issue_state's detection works.
	comment := botAlertSignature + ". Maintainers, please review."
	url := fmt.Sprintf("%s/repos/%s/%s/issues/%d/comments", githubBaseURL, owner, repo, in.ItemNumber)
	if _, err := postRequest(url, map[string]any{"body": comment}); err != nil {
		return errorResponse(fmt.Sprintf("Error posting alert: %v", err)), nil
	}
	return map[string]any{"status": "success"}, nil
}

func closeAsStale(_ agent.Context, in itemArgs) (map[string]any, error) {
	days := formatDays(closeHoursAfterStaleThreshold)
	comment := "This has been automatically closed because it has been marked as stale" +
		fmt.Sprintf(" for over %s days.", days)
	base := fmt.Sprintf("%s/repos/%s/%s/issues/%d", githubBaseURL, owner, repo, in.ItemNumber)
	if _, err := postRequest(base+"/comments", map[string]any{"body": comment}); err != nil {
		return errorResponse(fmt.Sprintf("Error closing issue: %v", err)), nil
	}
	if _, err := patchRequest(base, map[string]any{"state": "closed"}); err != nil {
		return errorResponse(fmt.Sprintf("Error closing issue: %v", err)), nil
	}
	return map[string]any{"status": "success"}, nil
}

// formatPrompt is PROMPT_TEMPLATE.format(...).
func formatPrompt() string {
	return strings.NewReplacer(
		"{OWNER}", owner,
		"{REPO}", repo,
		"{STALE_LABEL_NAME}", staleLabelName,
		"{REQUEST_CLARIFICATION_LABEL}", requestClarificationLabel,
		"{stale_threshold_days}", pyFloat(staleHoursThreshold/24),
		"{close_threshold_days}", pyFloat(closeHoursAfterStaleThreshold/24),
	).Replace(promptTemplate)
}

var (
	rootAgentOnce sync.Once
	rootAgent     agent.Agent
)

// newRootAgent is create_root_agent() + setup_secure_a2a(), built once.
func newRootAgent(ctx context.Context) agent.Agent {
	rootAgentOnce.Do(func() {
		rootAgent = team.RootAgent(ctx, llmagent.Config{
			Model:       team.Model(ctx),
			Name:        "adk_repository_auditor_agent",
			Description: "Audits open issues.",
			Instruction: formatPrompt(),
			Tools: []tool.Tool{
				team.SecuredTool("add_label_to_issue", `
  Adds a label to the issue.

  Args:
      item_number (int): The GitHub issue number.
      label_name (str): The name of the label to add.
  `, addLabelToIssue),
				team.SecuredTool("add_stale_label_and_comment", `
  Marks the issue as stale with a comment and label.

  Args:
      item_number (int): The GitHub issue number.
  `, addStaleLabelAndComment),
				team.SecuredTool("alert_maintainer_of_edit", `
  Posts a comment alerting maintainers of a silent description update.

  Args:
      item_number (int): The GitHub issue number.
  `, alertMaintainerOfEdit),
				team.SecuredTool("close_as_stale", `
  Closes the issue as not planned/stale.

  Args:
      item_number (int): The GitHub issue number.
  `, closeAsStale),
				team.SecuredTool("get_issue_state", `
  Retrieves the comprehensive state of a GitHub issue using GraphQL.

  This function orchestrates the fetching, parsing, and analysis of the issue's
  history to determine if it is stale, active, or pending maintainer review.

  Args:
      item_number (int): The GitHub issue number.

  Returns:
      Dict[str, Any]: A comprehensive state dictionary for the LLM agent.
          Contains keys such as 'last_action_role', 'is_stale', 'days_since_activity',
          and 'maintainer_alert_needed'.
  `, getIssueState),
				team.SecuredTool("remove_label_from_issue", `
  Removes a label from the issue.

  Args:
      item_number (int): The GitHub issue number.
      label_name (str): The name of the label to remove.
  `, removeLabelFromIssue),
			},
		})
	})
	return rootAgent
}
