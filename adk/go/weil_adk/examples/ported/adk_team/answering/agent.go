package main

import (
	"context"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/geminiassistant"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

func approvalInstruction() string {
	if isInteractive {
		return "Ask for user approval or confirmation for adding the comment."
	}
	return "**Do not** wait or ask for user approval or confirmation for adding the" +
		" comment."
}

// newRootAgent is agent.py's create_root_agent + setup_secure_a2a.
func newRootAgent(ctx context.Context) agent.Agent {
	return team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "adk_answering_agent",
		Description: "Answer questions about ADK repo.",
		Instruction: instruction(),
		Tools: []tool.Tool{
			// VertexAiSearchTool(bypass_multi_tools_limit=True) among other
			// tools is what Python ADK turns into discovery_engine_search.
			team.DiscoveryEngineSearch(vertexAIDatastoreID),
			team.AgentTool(geminiassistant.New(ctx)),
			team.SecuredTool("get_discussion_and_comments", `Fetches a discussion and its comments using the GitHub GraphQL API.

Args:
    discussion_number: The number of the GitHub discussion.

Returns:
    A dictionary with the request status and the discussion details.`, getDiscussionAndComments),
			team.SecuredTool("add_comment_to_discussion", `Adds a comment to a specific discussion.

Args:
    discussion_id: The GraphQL node ID of the discussion.
    comment_body: The content of the comment in Markdown.

Returns:
    The status of the request and the new comment's details.`, addCommentToDiscussion),
			team.SecuredTool("add_label_to_discussion", `Adds a label to a specific discussion.

Args:
    discussion_id: The GraphQL node ID of the discussion.
    label_name: The name of the label to add (e.g., "bug").

Returns:
    The status of the request and the label details.`, addLabelToDiscussion),
			team.SecuredTool("convert_gcs_links_to_https", `Converts GCS files link into publicly accessible HTTPS links.

Args:
    gcs_uris: A list of GCS files links, in the format
      'gs://bucket_name/prefix/relative_path'.

Returns:
    A dictionary mapping the original GCS files links to the converted HTTPS
    links. If a GCS link is invalid, the corresponding value in the dictionary
    will be None.`, convertGCSLinksToHTTPS),
		},
	})
}
