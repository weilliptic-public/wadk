// Command gemini_assistant serves adk_answering_agent/gemini_assistant on
// its own, as `adk run` / `adk web` would load the Python module:
//
//	go run ./examples/ported/adk_team/gemini_assistant console
package main

import (
	"context"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/geminiassistant"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

func main() {
	ctx := context.Background()
	team.LoadDotEnv("")
	// adk_answering_agent.settings, which gemini_assistant imports, requires both.
	team.MustGetenv("GITHUB_TOKEN")
	team.MustGetenv("VERTEXAI_DATASTORE_ID")
	team.Launch(ctx, geminiassistant.New(ctx))
}
