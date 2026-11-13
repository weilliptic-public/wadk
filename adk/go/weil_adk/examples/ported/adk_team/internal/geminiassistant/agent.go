// Package geminiassistant is adk_answering_agent/gemini_assistant: an agent
// answering questions about the Gemini API from a Vertex AI Search data
// store, with the team identity of its own. adk_answering_agent uses it
// through AgentTool.
package geminiassistant

import (
	"context"
	"fmt"
	"os"

	"golang.org/x/oauth2/google"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

type searchArgs struct {
	Queries []string `json:"queries"`
}

func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "error_message": msg}
}

// searchGeminiAPIDocs is search_gemini_api_docs.
func searchGeminiAPIDocs(ctx agent.Context, in searchArgs) (map[string]any, error) {
	key := os.Getenv("ADK_GCP_SA_KEY")
	if key == "" {
		// json.loads(None)
		return errorResponse("Error creating Vertex AI Search client: the JSON object must be str, bytes or bytearray, not NoneType"), nil
	}
	creds, err := google.CredentialsFromJSON(ctx, []byte(key), "https://www.googleapis.com/auth/cloud-platform")
	if err != nil {
		return errorResponse(fmt.Sprintf("Error creating Vertex AI Search client: %v", err)), nil
	}

	servingConfig := os.Getenv("GEMINI_API_DATASTORE_ID") + "/servingConfigs/default_config"
	results := []any{}
	for _, query := range in.Queries {
		resp, err := team.DiscoverySearch(ctx, creds.TokenSource, "discoveryengine.googleapis.com", "", servingConfig,
			map[string]any{"query": query, "pageSize": 20})
		if err != nil {
			return errorResponse(fmt.Sprintf("Error from Vertex AI Search: %v", err)), nil
		}
		for _, item := range team.List(resp["results"]) {
			derived := team.Map(team.Get(item, "document", "derivedStructData"))
			snippets := []any{}
			for _, s := range team.List(derived["snippets"]) {
				snippets = append(snippets, team.Get(s, "snippet"))
			}
			results = append(results, map[string]any{
				"title":    derived["title"],
				"url":      derived["link"],
				"snippets": snippets,
			})
		}
	}
	return map[string]any{"status": "success", "results": results}, nil
}

const instruction = `
    You are a helpful assistant that responds to questions about Gemini API based on information
    found in the document store. You can access the document store using the ` + "`search_gemini_api_docs`" + ` tool.

    When user asks a question, here are the steps:
    1. Use the ` + "`search_gemini_api_docs`" + ` tool to find relevant information before answering.
      * You can call the tool with multiple queries to find all the relevant information.
    2. Provide a response based on the information you found in the document store. Reference the source document in the response.

    IMPORTANT:
      * Your response should be based on the information you found in the document store. Do not invent
        information that is not in the document store. Do not invent citations which are not in the document store.
      * If you can't find the answer or information in the document store, just respond with "I can't find the answer or information in the document store".
      * If you uses citation from the document store, please always provide a footnote referencing the source document format it as: "[1] URL of the document".
    `

// New builds gemini_assistant: the team identity, its own setup_secure_a2a
// (Pensieve tools; audits and traces come from the runner's team.Plugin, or
// team.AgentTool's when used as a tool).
func New(ctx context.Context) agent.Agent {
	return team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "gemini_assistant",
		Description: "Answer questions about Gemini API.",
		Instruction: instruction,
		Tools: []tool.Tool{
			team.SecuredTool("search_gemini_api_docs", `Searches Gemini API docs using Vertex AI Search.

Args:
  queries: The list of queries to search.

Returns:
  A dictionary containing the status of the request and the list of search
  results, which contains the title, url and snippets.`, searchGeminiAPIDocs),
		},
	})
}
