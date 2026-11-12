// Command answering is adk_answering_agent (main.py): it answers questions
// in GitHub discussions of OWNER/REPO, as the team identity, with its
// secured tools audited and each run traced on chain.
//
//	go run ./examples/ported/adk_team/answering --recent 10
//	go run ./examples/ported/adk_team/answering --discussion_number 21
//	go run ./examples/ported/adk_team/answering --discussion '{"number": 21, "title": "...", "body": "..."}'
//	go run ./examples/ported/adk_team/answering --discussion-file event.json
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io/fs"
	"os"
	"strings"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "adk_answering_app"
	userID  = "adk_answering_user"
)

type args struct {
	recent           int
	discussionNumber string
	discussion       string
	discussionFile   string
}

func processArguments() args {
	var a args
	fl := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	fl.Usage = func() {
		fmt.Fprintf(fl.Output(), "usage: %s [-h] (--recent COUNT | --discussion_number NUM | --discussion JSON | --discussion-file FILE)\n\n", os.Args[0])
		fmt.Fprintln(fl.Output(), "A script that answers questions for GitHub discussions.")
		fl.PrintDefaults()
		fmt.Fprintf(fl.Output(), "\nExample usage: \n\t%[1]s --recent 10\n\t%[1]s --discussion_number 21\n\t%[1]s --discussion '{\"number\": 21, \"title\": \"...\", \"body\": \"...\"}'\n", os.Args[0])
	}
	fl.IntVar(&a.recent, "recent", 0, "Answer the N most recently updated discussion numbers.")
	fl.StringVar(&a.discussionNumber, "discussion_number", "", "Answer a specific discussion number.")
	fl.StringVar(&a.discussion, "discussion", "", "Answer a discussion using provided JSON data from GitHub event.")
	fl.StringVar(&a.discussionFile, "discussion-file", "", "Answer a discussion using JSON data from a file.")
	_ = fl.Parse(os.Args[1:])
	set := 0
	fl.Visit(func(*flag.Flag) { set++ })
	if set != 1 {
		fl.Usage()
		if set == 0 {
			fmt.Fprintf(os.Stderr, "%s: error: one of the arguments --recent --discussion_number --discussion --discussion-file is required\n", os.Args[0])
		} else {
			fmt.Fprintf(os.Stderr, "%s: error: the arguments --recent --discussion_number --discussion --discussion-file are mutually exclusive\n", os.Args[0])
		}
		os.Exit(2)
	}
	return a
}

func run(ctx context.Context) {
	a := processArguments()
	var discussionNumbers []any
	var discussionJSON []byte

	switch {
	case a.recent != 0:
		fetched := listMostRecentDiscussions(a.recent)
		if len(fetched) == 0 {
			fmt.Fprintln(os.Stderr, "No discussions found. Exiting...")
			return
		}
		for _, n := range fetched {
			discussionNumbers = append(discussionNumbers, n)
		}
	case a.discussionNumber != "":
		n := team.ParseNumberString(a.discussionNumber, 0)
		if n == 0 {
			fmt.Printf("Error: Invalid discussion number received: %s.\n", a.discussionNumber)
			return
		}
		discussionNumbers = []any{n}
	case a.discussion != "" || a.discussionFile != "":
		var raw []byte
		var sourceDesc string
		if a.discussion != "" {
			raw, sourceDesc = []byte(a.discussion), "--discussion argument"
		} else {
			b, err := os.ReadFile(a.discussionFile)
			if errors.Is(err, fs.ErrNotExist) {
				fmt.Fprintf(os.Stderr, "Error: File not found: %s\n", a.discussionFile)
				return
			} else if err != nil {
				team.Fatalf("%v", err)
			}
			raw, sourceDesc = b, "file "+a.discussionFile
		}
		var data map[string]any
		if err := json.Unmarshal(raw, &data); err != nil {
			fmt.Fprintf(os.Stderr, "Error: Invalid JSON in %s: %v\n", sourceDesc, err)
			return
		}
		number := data["number"]
		if !team.Truthy(number) {
			fmt.Fprintln(os.Stderr, "Error: Discussion JSON missing 'number' field.")
			return
		}
		discussionNumbers = []any{number}
		discussionJSON = raw
	}

	fmt.Printf("Will try to answer discussions: %s...\n", team.Repr(discussionNumbers))

	runner := team.NewRunner(appName, newRootAgent(ctx))

	for _, number := range discussionNumbers {
		n := team.PyStr(number)
		if len(discussionNumbers) > 1 {
			fmt.Println(strings.Repeat("#", 80))
			fmt.Printf("Starting to process discussion #%s...\n", n)
		}
		sessionID, err := runner.NewSession(ctx, userID)
		if err != nil {
			team.Fatalf("%v", err)
		}

		var prompt string
		if discussionJSON != nil {
			pretty, err := team.DumpsIndent(discussionJSON)
			if err != nil {
				team.Fatalf("%v", err)
			}
			prompt = fmt.Sprintf("Please help answer this GitHub discussion #%s."+
				" Here is the complete discussion"+
				" data:\n\n```json\n%s\n```\n\nPlease analyze"+
				" this discussion and provide a helpful response based on your"+
				" knowledge of ADK.", n, pretty)
		} else {
			prompt = fmt.Sprintf("Please check discussion #%s see if you can help"+
				" answer the question or provide some information!", n)
		}

		response, err := runner.CallAgent(ctx, userID, sessionID, prompt)
		if err != nil {
			team.Fatalf("%v", err)
		}
		fmt.Printf("<<<< Agent Final Output: %s\n\n", response)
	}
}

func main() {
	loadSettings()
	team.SetupLogger(team.DEBUG)
	ctx := context.Background()

	start := time.Now()
	fmt.Printf("Start Q&A checking on %s/%s at %s\n", owner, repo, team.TimeLine(start))
	fmt.Println(strings.Repeat("-", 80))
	run(ctx)
	fmt.Println(strings.Repeat("-", 80))
	end := time.Now()
	fmt.Printf("Q&A checking finished at %s\n", team.TimeLine(end))
	fmt.Printf("Total script execution time: %.2f seconds\n", end.Sub(start).Seconds())
}
