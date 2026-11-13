// Command stale is the ported adk_stale_agent: it audits open issues older
// than STALE_HOURS_THRESHOLD, marking ones awaiting the author's reply as
// stale and closing them after CLOSE_HOURS_AFTER_STALE_THRESHOLD, as the
// team identity with every tool secured under WEIL_ADK_TEAM_SERVICE.
//
// Run from go/weil_adk: go run ./examples/ported/adk_team/stale
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/session"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "stale_bot_app"
	userID  = "stale_bot_user"
)

type issueResult struct {
	duration  float64
	apiCalls  int
	succeeded bool
}

// processSingleIssue audits one issue; a failure is reported through the
// result so one failing issue does not stop the batch.
func processSingleIssue(ctx context.Context, issueNumber int) issueResult {
	start := time.Now()
	startAPICalls := getAPICallCount()
	succeeded := true

	team.Log.Infof("Processing Issue #%d...", issueNumber)
	team.Log.Debugf("#%d: Initializing runner and session.", issueNumber)

	err := func() error {
		r := team.NewRunner(appName, newRootAgent(ctx))
		sessionID, err := r.NewSession(ctx, userID)
		if err != nil {
			return err
		}
		promptText := fmt.Sprintf("Audit Issue #%d.", issueNumber)
		team.Log.Debugf("#%d: Sending prompt to agent.", issueNumber)
		return r.Send(ctx, userID, sessionID, promptText, func(ev *session.Event) error {
			if ev.Content != nil && len(ev.Content.Parts) > 0 && ev.Content.Parts[0] != nil {
				if text := ev.Content.Parts[0].Text; text != "" {
					clean := strings.ReplaceAll(firstRunes(text, 150), "\n", " ")
					team.Log.Infof("#%d Decision: %s...", issueNumber, clean)
				}
			}
			return nil
		})
	}()
	if err != nil {
		team.Log.Errorf("Error processing issue #%d: %v", issueNumber, err)
		succeeded = false
	}

	duration := time.Since(start).Seconds()
	issueAPICalls := getAPICallCount() - startAPICalls
	verb := "finished"
	if !succeeded {
		verb = "failed"
	}
	team.Log.Infof("Issue #%d %s in %.2fs with ~%d API calls.", issueNumber, verb, duration, issueAPICalls)
	return issueResult{duration, issueAPICalls, succeeded}
}

func firstRunes(s string, n int) string {
	r := []rune(s)
	if len(r) > n {
		return string(r[:n])
	}
	return s
}

// run fetches old issues and processes them CONCURRENCY_LIMIT at a time.
// It returns 0 if every issue was audited, 1 if any failed or the run could
// not start.
func run(ctx context.Context) int {
	team.Log.Infof("--- Starting Stale Bot for %s/%s ---", owner, repo)
	team.Log.Infof("Concurrency level set to %d", concurrencyLimit)

	resetAPICallCount()

	filterDays := staleHoursThreshold / 24
	team.Log.Debugf("Fetching issues older than %.2f days...", filterDays)

	allIssues, err := getOldOpenIssueNumbers(owner, repo, filterDays)
	if err != nil {
		team.Log.Criticalf("Failed to fetch issue list: %v", err)
		return 1
	}
	totalCount := len(allIssues)
	searchAPICalls := getAPICallCount()

	if totalCount == 0 {
		team.Log.Infof("No issues matched the criteria. Run finished.")
		return 0
	}
	team.Log.Infof("Found %d issues to process. (Initial search used %d API calls).", totalCount, searchAPICalls)

	totalProcessingTime := 0.0
	totalIssueAPICalls := 0
	succeededCount, failedCount := 0, 0

	for i := 0; i < totalCount; i += concurrencyLimit {
		chunk := allIssues[i:min(i+concurrencyLimit, totalCount)]
		chunkNum := i/concurrencyLimit + 1
		team.Log.Infof("--- Starting chunk %d: Processing issues %s ---", chunkNum, intList(chunk))

		results := make([]issueResult, len(chunk))
		var wg sync.WaitGroup
		for j, n := range chunk {
			wg.Go(func() { results[j] = processSingleIssue(ctx, n) })
		}
		wg.Wait()

		for _, r := range results {
			totalProcessingTime += r.duration
			totalIssueAPICalls += r.apiCalls
			if r.succeeded {
				succeededCount++
			} else {
				failedCount++
			}
		}
		team.Log.Infof("--- Finished chunk %d. Progress: %d/%d ---", chunkNum, succeededCount+failedCount, totalCount)

		if i+concurrencyLimit < totalCount {
			team.Log.Debugf("Sleeping for %ss to respect rate limits...", pyFloat(sleepBetweenChunks))
			time.Sleep(time.Duration(sleepBetweenChunks * float64(time.Second)))
		}
	}

	totalAPICallsForRun := searchAPICalls + totalIssueAPICalls
	avgTimePerIssue := totalProcessingTime / float64(totalCount)

	team.Log.Infof("--- Stale Agent Run Finished ---")
	team.Log.Infof("Successfully processed %d issues.", succeededCount)
	if failedCount > 0 {
		team.Log.Errorf("Failed to process %d issues.", failedCount)
	}
	team.Log.Infof("Total API calls made this run: %d", totalAPICallsForRun)
	team.Log.Infof("Average processing time per issue: %.2f seconds.", avgTimePerIssue)

	if failedCount > 0 {
		return 1
	}
	return 0
}

// intList formats ints as a Python list: [1, 2, 3].
func intList(ns []int) string {
	parts := make([]string, len(ns))
	for i, n := range ns {
		parts[i] = fmt.Sprint(n)
	}
	return "[" + strings.Join(parts, ", ") + "]"
}

func main() {
	team.SetupLogger(team.INFO)
	loadSettings()
	start := time.Now()

	// Anything short of a completed run is a failure, so a scheduled run
	// that audited nothing does not look healthy.
	exitCode := 1

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	done := make(chan int, 1)
	go func() {
		defer func() {
			if r := recover(); r != nil {
				team.Log.Criticalf("Unexpected fatal error: %v", r)
				done <- 1
			}
		}()
		done <- run(ctx)
	}()
	select {
	case exitCode = <-done:
	case <-ctx.Done():
		team.Log.Warningf("Bot execution interrupted manually.")
	}
	stop()

	team.Log.Infof("Full audit finished in %.2f minutes.", time.Since(start).Minutes())
	os.Exit(exitCode)
}
