// Command issue_monitoring is adk_issue_monitoring_agent (main.py): it
// reads the non-maintainer text of recently updated open issues of
// OWNER/REPO and has spam_auditor_agent alert maintainers to spam, as the
// team identity, with its secured tool audited and each run traced.
//
//	go run ./examples/ported/adk_team/issue_monitoring
package main

import (
	"context"
	"fmt"
	"os"
	"regexp"
	"slices"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/session"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "issue_monitoring_app"
	userID  = "issue_monitoring_user"
)

type outcome string

const (
	audited outcome = "audited"
	skipped outcome = "skipped"
	failed  outcome = "failed"
)

var codeBlock = regexp.MustCompile("(?s)```.*?```")

func clean(body string) string {
	c := codeBlock.ReplaceAllString(body, "\n[CODE BLOCK REMOVED]\n")
	if r := []rune(c); len(r) > 1500 {
		c = string(r[:1500]) + "\n...[TRUNCATED]"
	}
	return c
}

func isMaintainerOrBot(author string, maintainers []string) bool {
	return slices.Contains(maintainers, author) || strings.HasSuffix(author, "[bot]") || author == botName
}

func processSingleIssue(ctx context.Context, runner *team.Runner, issueNumber int, maintainers []string) (time.Duration, int64, outcome) {
	start := time.Now()
	startCalls := getAPICallCount()
	result := func(o outcome) (time.Duration, int64, outcome) {
		return time.Since(start), getAPICallCount() - startCalls, o
	}

	err := func() error {
		issue, err := getIssueDetails(owner, repo, issueNumber)
		if err != nil {
			return err
		}
		comments, err := getIssueComments(owner, repo, issueNumber)
		if err != nil {
			return err
		}

		var userComments []string
		issueAuthor := team.Str(team.Get(issue, "user", "login"))
		issueBody := team.Str(issue["body"])
		if !isMaintainerOrBot(issueAuthor, maintainers) {
			userComments = append(userComments, fmt.Sprintf("Author (Original Issue): @%s\nText: %s\n---", issueAuthor, clean(issueBody)))
		}

		for _, c := range comments {
			author := team.Str(team.Get(c, "user", "login"))
			body := team.Str(team.Get(c, "body"))
			if strings.Contains(body, botAlertSignature) {
				log.Infof("#%d: Spam bot already alerted maintainers previously. Skipping.", issueNumber)
				return errSkipped
			}
			if isMaintainerOrBot(author, maintainers) {
				continue
			}
			userComments = append(userComments, fmt.Sprintf("Author: @%s\nComment: %s\n---", author, clean(body)))
		}

		if len(userComments) == 0 {
			log.Debugf("#%d: No non-maintainer text found. Skipping.", issueNumber)
			return errSkipped
		}

		log.Infof("Processing Issue #%d (Found %d items to review)...", issueNumber, len(userComments))
		prompt := fmt.Sprintf("Please review the following text for issue #%d:\n\n%s", issueNumber, strings.Join(userComments, "\n"))

		sessionID, err := runner.NewSession(ctx, userID)
		if err != nil {
			return err
		}
		return runner.Send(ctx, userID, sessionID, prompt, func(ev *session.Event) error {
			if ev.Content != nil && len(ev.Content.Parts) > 0 && ev.Content.Parts[0] != nil {
				if text := ev.Content.Parts[0].Text; text != "" {
					r := []rune(text)
					if len(r) > 100 {
						r = r[:100]
					}
					log.Infof("#%d Decision: %s...", issueNumber, strings.ReplaceAll(string(r), "\n", " "))
				}
			}
			return nil
		})
	}()

	switch {
	case err == errSkipped:
		return result(skipped)
	case err != nil:
		log.Errorf("Error processing issue #%d: %v", issueNumber, err)
		return result(failed)
	}
	return result(audited)
}

type skip struct{}

func (skip) Error() string { return "skipped" }

var errSkipped error = skip{}

func run(ctx context.Context) int {
	log.Infof("--- Starting Issue Monitoring Agent for %s/%s ---", owner, repo)
	resetAPICallCount()

	maintainers, err := getRepositoryMaintainers(owner, repo)
	if err != nil {
		log.Criticalf("Failed to fetch maintainers: %v", err)
		return 1
	}
	log.Infof("Found %d maintainers.", len(maintainers))

	allIssues, err := getTargetIssues(owner, repo)
	if err != nil {
		log.Criticalf("Failed to fetch issue list: %v", err)
		return 1
	}
	total := len(allIssues)
	if total == 0 {
		log.Infof("No issues matched criteria. Run finished.")
		return 0
	}
	log.Infof("Found %d issues to process.", total)

	runner := team.NewRunner(appName, newRootAgent(ctx))

	var auditedCount, skippedCount, failedCount int
	for i := 0; i < total; i += concurrencyLimit {
		chunk := allIssues[i:min(i+concurrencyLimit, total)]
		log.Infof("Processing chunk: %s", team.Repr(toAny(chunk)))

		outcomes := make([]outcome, len(chunk))
		var wg sync.WaitGroup
		for j, n := range chunk {
			wg.Add(1)
			go func() {
				defer wg.Done()
				_, _, outcomes[j] = processSingleIssue(ctx, runner, n, maintainers)
			}()
		}
		wg.Wait()

		for _, o := range outcomes {
			switch o {
			case audited:
				auditedCount++
			case skipped:
				skippedCount++
			default:
				failedCount++
			}
		}
		if i+concurrencyLimit < total {
			time.Sleep(time.Duration(sleepBetweenChunks * float64(time.Second)))
		}
	}

	log.Infof("--- Run Finished. Total API calls: %d ---", getAPICallCount())
	log.Infof("Successfully processed %d issues.", auditedCount)
	if skippedCount > 0 {
		log.Infof("Skipped %d issues.", skippedCount)
	}
	if failedCount > 0 {
		log.Errorf("Failed to process %d issues.", failedCount)
		return 1
	}
	return 0
}

func toAny(ns []int) []any {
	out := make([]any, len(ns))
	for i, n := range ns {
		out[i] = n
	}
	return out
}

func main() {
	loadSettings()
	team.SetupLogger(team.INFO)
	os.Exit(run(context.Background()))
}
