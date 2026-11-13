// Command release_analyzer is the Go port of adk_team's
// adk_documentation/adk_release_analyzer (agent.py and main.py):
// a SequentialAgent + LoopAgent pipeline that analyzes the changes between
// two ADK Python releases and files a docs-update issue.
//
// Agents:
//
//	adk_release_analyzer (root, team identity, + Pensieve tools)
//	└── analysis_pipeline (SequentialAgent)
//	    ├── release_planner
//	    ├── file_analysis_loop (LoopAgent, max 50) └── file_group_analyzer
//	    └── summary_agent
//
// State keys: start_tag, end_tag, compare_url, file_groups,
// current_group_index, recommendations, release_summary, all_changed_files.
//
// Usage:
//
//	go run ./examples/ported/adk_team/release_analyzer [--resume] [--start-tag v1.26.0] [--end-tag v1.27.0]
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/workflowagents/loopagent"
	"google.golang.org/adk/v2/agent/workflowagents/sequentialagent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/exitlooptool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/docs"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

const (
	appName = "adk_release_analyzer"
	userID  = "adk_release_analyzer_user"

	// maxFilesPerGroup is the maximum number of files per analysis group,
	// to avoid context overflow.
	maxFilesPerGroup = 5
)

func approvalInstruction() string {
	if docs.IsInteractive {
		return "Ask for user approval or confirmation for creating or updating the issue."
	}
	return "**Do not** wait or ask for user approval or confirmation for creating or updating the issue."
}

// fill substitutes the f-string fields of an instruction; extra holds the
// state-derived ones of the dynamic instructions.
func fill(s string, extra ...string) string {
	pairs := []string{
		"{CODE_OWNER}", docs.CodeOwner,
		"{CODE_REPO}", docs.CodeRepo,
		"{DOC_OWNER}", docs.DocOwner,
		"{DOC_REPO}", docs.DocRepo,
		"{LOCAL_REPOS_DIR_PATH}", docs.LocalReposDirPath,
		"{MAX_FILES_PER_GROUP}", strconv.Itoa(maxFilesPerGroup),
		"{APPROVAL_INSTRUCTION}", approvalInstruction(),
	}
	return strings.NewReplacer(append(pairs, extra...)...).Replace(s)
}

// =============================================================================
// Tool functions for state management
// =============================================================================

// stateGet is tool_context.state.get(key, def).
func stateGet(st interface{ Get(string) (any, error) }, key string, def any) any {
	v, err := st.Get(key)
	if err != nil {
		return def
	}
	return v
}

// generic converts v to the JSON-decoded form (maps, []any, float64) that
// state holds after a reload, so reads see one shape.
func generic(v any) any {
	b, err := json.Marshal(v)
	if err != nil {
		return v
	}
	var out any
	if json.Unmarshal(b, &out) != nil {
		return v
	}
	return out
}

func newGetNextFileGroup() tool.Tool {
	return team.Tool("get_next_file_group", `Gets the next group of files to analyze from the state.

This tool retrieves the next file group from state["file_groups"]
and increments the current_group_index.

Args:
    tool_context: The tool context providing access to state.

Returns:
    A dictionary with the next file group or indication that all groups
    are processed.`,
		func(ctx agent.Context, _ struct{}) (map[string]any, error) {
			fileGroups := team.List(stateGet(ctx.State(), "file_groups", []any{}))
			currentIndex := team.Int(stateGet(ctx.State(), "current_group_index", 0))

			if currentIndex >= len(fileGroups) {
				fmt.Printf("[Progress] All %d groups processed.\n", len(fileGroups))
				return map[string]any{
					"status":       "complete",
					"message":      "All file groups have been processed.",
					"total_groups": len(fileGroups),
					"processed":    currentIndex,
				}, nil
			}

			currentGroup := fileGroups[currentIndex]
			var paths []string
			for _, f := range team.List(currentGroup) {
				p, ok := team.Map(f)["relative_path"]
				if !ok {
					p = "?"
				}
				paths = append(paths, team.PyStr(p))
			}
			fmt.Printf("[Progress] Starting group %d/%d: %s\n", currentIndex+1, len(fileGroups), team.Repr(paths))

			return map[string]any{
				"status":       "success",
				"group_index":  currentIndex,
				"total_groups": len(fileGroups),
				"remaining":    len(fileGroups) - currentIndex - 1,
				"files":        currentGroup,
			}, nil
		})
}

func newSaveGroupRecommendations() tool.Tool {
	type args struct {
		GroupIndex      int              `json:"group_index"`
		Recommendations []map[string]any `json:"recommendations"`
	}
	return team.Tool("save_group_recommendations", `Saves recommendations for a file group to state.

Args:
    tool_context: The tool context providing access to state.
    group_index: The index of the group these recommendations belong to.
    recommendations: List of recommendation dicts with keys:
        - summary: Brief summary of the change
        - doc_file: Path to the doc file to update
        - current_state: Current content in the doc
        - proposed_change: What should be changed
        - reasoning: Why this change is needed
        - reference: Reference to the code file

Returns:
    A dictionary confirming the save operation.`,
		func(ctx agent.Context, in args) (map[string]any, error) {
			all := append([]any{}, team.List(stateGet(ctx.State(), "recommendations", []any{}))...)
			all = append(all, team.List(generic(in.Recommendations))...)
			if err := ctx.State().Set("recommendations", all); err != nil {
				return nil, err
			}
			// Advance index only after recommendations are saved, so interrupted
			// groups get retried on resume instead of being skipped.
			if err := ctx.State().Set("current_group_index", in.GroupIndex+1); err != nil {
				return nil, err
			}

			totalGroups := len(team.List(stateGet(ctx.State(), "file_groups", []any{})))
			fmt.Printf("[Progress] Group %d/%d done. +%d recommendations (%d total)\n",
				in.GroupIndex+1, totalGroups, len(in.Recommendations), len(all))

			return map[string]any{
				"status":                "success",
				"group_index":           in.GroupIndex,
				"new_recommendations":   len(in.Recommendations),
				"total_recommendations": len(all),
			}, nil
		})
}

func newGetAllRecommendations() tool.Tool {
	return team.Tool("get_all_recommendations", `Retrieves all accumulated recommendations from state.

Args:
    tool_context: The tool context providing access to state.

Returns:
    A dictionary with all recommendations and metadata.`,
		func(ctx agent.Context, _ struct{}) (map[string]any, error) {
			recs := team.List(stateGet(ctx.State(), "recommendations", []any{}))
			startTag := stateGet(ctx.State(), "start_tag", "unknown")
			endTag := stateGet(ctx.State(), "end_tag", "unknown")
			compareURL := stateGet(ctx.State(), "compare_url", "")

			fmt.Printf("[Summary] Retrieving recommendations: %d total, release %s → %s\n",
				len(recs), team.PyStr(startTag), team.PyStr(endTag))

			if recs == nil {
				recs = []any{}
			}
			return map[string]any{
				"status":                "success",
				"start_tag":             startTag,
				"end_tag":               endTag,
				"compare_url":           compareURL,
				"total_recommendations": len(recs),
				"recommendations":       recs,
			}, nil
		})
}

func newSaveReleaseInfo() tool.Tool {
	type args struct {
		StartTag        string             `json:"start_tag"`
		EndTag          string             `json:"end_tag"`
		CompareURL      string             `json:"compare_url"`
		FileGroups      [][]map[string]any `json:"file_groups"`
		ReleaseSummary  string             `json:"release_summary"`
		AllChangedFiles []string           `json:"all_changed_files"`
	}
	return team.Tool("save_release_info", `Saves release info and file groups to state for processing.

Args:
    tool_context: The tool context providing access to state.
    start_tag: The starting release tag.
    end_tag: The ending release tag.
    compare_url: The GitHub compare URL.
    file_groups: List of file groups, where each group is a list of file
        info dicts.
    release_summary: A high-level summary of all changes in this release,
        including the main themes (e.g., "new feature X", "refactoring Y",
        "bug fixes in Z"). This helps individual analyzers understand the
        bigger picture.
    all_changed_files: List of all changed file paths (for cross-reference).

Returns:
    A dictionary confirming the save operation.`,
		func(ctx agent.Context, in args) (map[string]any, error) {
			set := map[string]any{
				"start_tag":           in.StartTag,
				"end_tag":             in.EndTag,
				"compare_url":         in.CompareURL,
				"file_groups":         generic(in.FileGroups),
				"current_group_index": 0,
				"recommendations":     []any{},
				"release_summary":     in.ReleaseSummary,
				"all_changed_files":   generic(in.AllChangedFiles),
			}
			for _, k := range []string{"start_tag", "end_tag", "compare_url", "file_groups",
				"current_group_index", "recommendations", "release_summary", "all_changed_files"} {
				if err := ctx.State().Set(k, set[k]); err != nil {
					return nil, err
				}
			}

			total := 0
			for _, g := range in.FileGroups {
				total += len(g)
			}
			fmt.Printf("[Planning] Release %s → %s: %d files in %d groups\n", in.StartTag, in.EndTag, total, len(in.FileGroups))

			return map[string]any{
				"status":       "success",
				"start_tag":    in.StartTag,
				"end_tag":      in.EndTag,
				"total_groups": len(in.FileGroups),
				"total_files":  total,
			}, nil
		})
}

func newGetReleaseContext() tool.Tool {
	return team.Tool("get_release_context", `Gets the global release context for cross-group awareness.

This allows individual file group analyzers to understand:
- The overall theme of the release
- What other files were changed (for identifying related changes)
- What recommendations have already been made (to avoid duplicates)

Args:
    tool_context: The tool context providing access to state.

Returns:
    A dictionary with global release context.`,
		func(ctx agent.Context, _ struct{}) (map[string]any, error) {
			st := ctx.State()
			return map[string]any{
				"status":                   "success",
				"start_tag":                stateGet(st, "start_tag", "unknown"),
				"end_tag":                  stateGet(st, "end_tag", "unknown"),
				"release_summary":          stateGet(st, "release_summary", ""),
				"all_changed_files":        stateGet(st, "all_changed_files", []any{}),
				"existing_recommendations": stateGet(st, "recommendations", []any{}),
				"current_group_index":      stateGet(st, "current_group_index", 0),
				"total_groups":             len(team.List(stateGet(st, "file_groups", []any{}))),
			}, nil
		})
}

// =============================================================================
// Agents
// =============================================================================

func must[T any](v T, err error) T {
	if err != nil {
		team.Fatalf("%v", err)
	}
	return v
}

// newPlannerAgent is Agent 1, release_planner.
func newPlannerAgent(m model.LLM) agent.Agent {
	return must(llmagent.New(llmagent.Config{
		Model:       m,
		Name:        "release_planner",
		Description: "Plans the analysis by fetching release info and organizing files into groups for incremental processing.",
		Instruction: fill(plannerInstruction),
		Tools: []tool.Tool{
			docs.CloneOrPullRepo(),
			docs.ListReleases(),
			docs.GetChangedFilesSummary(),
			newSaveReleaseInfo(),
		},
		OutputKey: "planner_output",
	}))
}

// fileAnalyzerInstruction is the dynamic instruction that includes current
// state info.
func fileAnalyzerInstructionProvider(ctx agent.ReadonlyContext) (string, error) {
	st := ctx.ReadonlyState()
	return fill(fileAnalyzerInstruction,
		"{start_tag}", team.PyStr(stateGet(st, "start_tag", "unknown")),
		"{end_tag}", team.PyStr(stateGet(st, "end_tag", "unknown")),
		"{release_summary}", team.PyStr(stateGet(st, "release_summary", "")),
	), nil
}

// newFileAnalysisLoop is Agent 2, file_group_analyzer, inside the LoopAgent
// that processes file groups one at a time.
func newFileAnalysisLoop(m model.LLM) agent.Agent {
	analyzer := must(llmagent.New(llmagent.Config{
		Model:               m,
		Name:                "file_group_analyzer",
		Description:         "Analyzes a group of changed files and generates recommendations.",
		InstructionProvider: fileAnalyzerInstructionProvider,
		IncludeContents:     llmagent.IncludeContentsNone,
		Tools: []tool.Tool{
			newGetNextFileGroup(),
			newGetReleaseContext(), // Get global context to avoid duplicates
			docs.GetFileDiffForRelease(),
			docs.SearchLocalGitRepo(),
			docs.ReadLocalGitRepoFileContent(),
			docs.ListDirectoryContents(),
			newSaveGroupRecommendations(),
			must(exitlooptool.New()), // Call this when all groups are processed
		},
		OutputKey: "analyzer_output",
	}))
	return must(loopagent.New(loopagent.Config{
		AgentConfig:   agent.Config{Name: "file_analysis_loop", SubAgents: []agent.Agent{analyzer}},
		MaxIterations: 50, // Safety limit
	}))
}

func summaryInstructionProvider(ctx agent.ReadonlyContext) (string, error) {
	st := ctx.ReadonlyState()
	return fill(summaryInstruction,
		"{start_tag}", team.PyStr(stateGet(st, "start_tag", "unknown")),
		"{end_tag}", team.PyStr(stateGet(st, "end_tag", "unknown")),
	), nil
}

// newSummaryAgent is Agent 3, summary_agent.
func newSummaryAgent(m model.LLM) agent.Agent {
	return must(llmagent.New(llmagent.Config{
		Model:               m,
		Name:                "summary_agent",
		Description:         "Compiles recommendations and creates the GitHub issue.",
		InstructionProvider: summaryInstructionProvider,
		IncludeContents:     llmagent.IncludeContentsNone,
		Tools:               []tool.Tool{newGetAllRecommendations(), docs.CreateIssue()},
		OutputKey:           "summary_output",
	}))
}

// newRootAgent is the entry point that understands user requests, with
// analysis_pipeline (planning, file analysis, summary) as its sub-agent.
// The sub-agents have no identity of their own: as in Python, they act as
// the root's wallet (weiladk resolves the nearest bound ancestor).
func newRootAgent(ctx context.Context, m model.LLM) agent.Agent {
	pipeline := must(sequentialagent.New(sequentialagent.Config{AgentConfig: agent.Config{
		Name:        "analysis_pipeline",
		Description: "Executes the release analysis pipeline: planning, file analysis, and summary generation.",
		SubAgents:   []agent.Agent{newPlannerAgent(m), newFileAnalysisLoop(m), newSummaryAgent(m)},
	}}))
	return team.RootAgent(ctx, llmagent.Config{
		Model:       m,
		Name:        "adk_release_analyzer",
		Description: "Analyzes ADK Python releases and generates documentation update recommendations.",
		Instruction: fill(rootInstruction),
		Tools: []tool.Tool{
			docs.CloneOrPullRepo(),
			docs.ListReleases(),
			docs.GetChangedFilesSummary(),
			docs.GetFileDiffForRelease(),
			docs.SearchLocalGitRepo(),
			docs.ReadLocalGitRepoFileContent(),
			newGetAllRecommendations(),
		},
		SubAgents: []agent.Agent{pipeline},
	})
}

// newResumePipeline resumes the pipeline from the file analysis loop,
// skipping the planner. Python builds it from deep copies made before
// setup_secure_a2a, so it carries no wallet and its secured tools are
// refused; here it is bound to the team identity so a resume can run.
func newResumePipeline(ctx context.Context, m model.LLM) agent.Agent {
	p := must(sequentialagent.New(sequentialagent.Config{AgentConfig: agent.Config{
		Name:        "resume_pipeline",
		Description: "Resumes the release analysis pipeline from the file analysis loop, skipping the planning phase.",
		SubAgents:   []agent.Agent{newFileAnalysisLoop(m), newSummaryAgent(m)},
	}}))
	return weiladk.Bind(p, team.Identity(ctx))
}

// =============================================================================
// Session persistence (Python: DatabaseSessionService on sqlite)
// =============================================================================

// sessionStore keeps each session's state in sessions.json beside this
// file, written after every event, so --resume can continue a run.
type sessionStore struct {
	path     string
	Sessions []storedSession `json:"sessions"`
}

type storedSession struct {
	ID    string         `json:"id"`
	State map[string]any `json:"state"`
}

func openStore() *sessionStore {
	_, file, _, _ := runtime.Caller(0)
	s := &sessionStore{path: filepath.Join(filepath.Dir(file), "sessions.json")}
	if b, err := os.ReadFile(s.path); err == nil {
		_ = json.Unmarshal(b, s)
	}
	return s
}

func (s *sessionStore) save(id string, state map[string]any) {
	found := false
	for i := range s.Sessions {
		if s.Sessions[i].ID == id {
			s.Sessions[i].State, found = state, true
		}
	}
	if !found {
		s.Sessions = append(s.Sessions, storedSession{ID: id, State: state})
	}
	if b, err := json.MarshalIndent(s, "", "  "); err == nil {
		if err := os.WriteFile(s.path, b, 0o600); err != nil {
			team.Log.Warningf("save session %s: %v", id, err)
		}
	}
}

func main() {
	resume := flag.Bool("resume", false, "Resume from the last session instead of starting fresh.")
	startTag := flag.String("start-tag", "", "The older release tag (base) for comparison, e.g. v1.26.0.")
	endTag := flag.String("end-tag", "", "The newer release tag (head) for comparison, e.g. v1.27.0.")
	flag.Parse()

	team.SetupLogger(team.INFO)
	start := time.Now()
	fmt.Printf("Start analyzing %s/%s releases for %s/%s updates at %s\n",
		docs.CodeOwner, docs.CodeRepo, docs.DocOwner, docs.DocRepo, team.TimeLine(start))
	fmt.Println(strings.Repeat("-", 80))
	if err := run(*resume, *startTag, *endTag); err != nil {
		team.Fatalf("%v", err)
	}
	fmt.Println(strings.Repeat("-", 80))
	end := time.Now()
	fmt.Printf("Triaging finished at %s\n", team.TimeLine(end))
	fmt.Printf("Total script execution time: %.2f seconds\n", end.Sub(start).Seconds())
}

func run(resume bool, startTag, endTag string) error {
	ctx := context.Background()
	m := team.Model(ctx)
	store := openStore()
	sessions := session.InMemoryService()

	if resume && len(store.Sessions) == 0 {
		fmt.Println("No previous session found. Starting fresh.")
		resume = false
	}

	var (
		r         *team.Runner
		sessionID string
		prompt    string
	)
	if resume {
		// Resume: use the last session with resume_pipeline (skip planner).
		last := store.Sessions[len(store.Sessions)-1]
		sessionID = last.ID
		if _, err := sessions.Create(ctx, &session.CreateRequest{
			AppName: appName, UserID: userID, SessionID: sessionID, State: last.State,
		}); err != nil {
			return err
		}
		state := last.State
		groupIndex := team.Int(getOr(state, "current_group_index", 0))
		totalGroups := len(team.List(getOr(state, "file_groups", []any{})))
		numRecs := len(team.List(getOr(state, "recommendations", []any{})))
		fmt.Printf("Resuming session %s\n", sessionID)
		fmt.Printf("  Progress: group %d/%d, %d recommendations so far\n", groupIndex+1, totalGroups, numRecs)
		fmt.Printf("  Release: %s → %s\n", team.PyStr(getOr(state, "start_tag", "?")), team.PyStr(getOr(state, "end_tag", "?")))

		r = team.NewRunnerWith(appName, newResumePipeline(ctx, m), sessions)
		prompt = "Resume analyzing the remaining file groups."
	} else {
		// Fresh run
		r = team.NewRunnerWith(appName, newRootAgent(ctx, m), sessions)
		id, err := r.NewSession(ctx, userID)
		if err != nil {
			return err
		}
		sessionID = id
		switch {
		case startTag != "" && endTag != "":
			prompt = fmt.Sprintf("Please analyze ADK Python releases from %s to %s!", startTag, endTag)
		case endTag != "":
			prompt = fmt.Sprintf("Please analyze the ADK Python release %s against its previous release!", endTag)
		default:
			prompt = "Please analyze the most recent two releases of ADK Python!"
		}
	}

	fmt.Printf("Session ID: %s\n", sessionID)
	fmt.Println(strings.Repeat("-", 80))

	var out strings.Builder
	err := r.Send(ctx, userID, sessionID, prompt, func(ev *session.Event) error {
		if ev.Content != nil && len(ev.Content.Parts) > 0 && ev.Author != "user" {
			out.WriteString(team.Text(ev.Content))
		}
		persist(ctx, sessions, store, sessionID)
		return nil
	})
	persist(ctx, sessions, store, sessionID)
	if err != nil {
		return err
	}
	fmt.Printf("<<<< Agent Final Output: %s\n\n", out.String())
	return nil
}

func persist(ctx context.Context, sessions session.Service, store *sessionStore, id string) {
	resp, err := sessions.Get(ctx, &session.GetRequest{AppName: appName, UserID: userID, SessionID: id})
	if err != nil {
		if !errors.Is(err, context.Canceled) {
			team.Log.Warningf("read session %s: %v", id, err)
		}
		return
	}
	state := map[string]any{}
	for k, v := range resp.Session.State().All() {
		state[k] = v
	}
	store.save(id, team.Map(generic(state)))
}

func getOr(m map[string]any, key string, def any) any {
	if v, ok := m[key]; ok {
		return v
	}
	return def
}
