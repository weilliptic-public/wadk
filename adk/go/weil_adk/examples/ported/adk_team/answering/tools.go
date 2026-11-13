package main

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// settings.py
var (
	githubBaseURL    = "https://api.github.com"
	githubGraphQLURL = githubBaseURL + "/graphql"

	githubToken         string
	vertexAIDatastoreID string

	owner, repo, botResponseLabel string
	isInteractive                 bool

	github *team.HTTP
)

func loadSettings() {
	team.LoadDotEnv("")
	githubToken = team.MustGetenv("GITHUB_TOKEN")
	vertexAIDatastoreID = team.MustGetenv("VERTEXAI_DATASTORE_ID")
	owner = team.Getenv("OWNER", "google")
	repo = team.Getenv("REPO", "adk-python")
	botResponseLabel = team.Getenv("BOT_RESPONSE_LABEL", "bot responded")
	isInteractive = team.IsTrue("INTERACTIVE", "1")
	github = &team.HTTP{Headers: team.GitHubHeaders(githubToken), Timeout: 60 * time.Second}
}

// utils.py

func errorResponse(msg string) map[string]any {
	return map[string]any{"status": "error", "error_message": msg}
}

func runGraphQLQuery(query string, variables map[string]any) (map[string]any, error) {
	return github.GraphQL(githubGraphQLURL, query, variables)
}

func checkURLExists(u string) bool {
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Head(u)
	if err != nil {
		return false
	}
	resp.Body.Close()
	return resp.StatusCode < 400
}

func generateGitHubURL(repoName, relativePath string) string {
	return fmt.Sprintf("https://github.com/google/%s/blob/main/%s", repoName, relativePath)
}

// convertGCSToHTTPS is convert_gcs_to_https; ok is false where Python
// returns None.
func convertGCSToHTTPS(gcsURI string) (string, bool) {
	if gcsURI == "" || !strings.HasPrefix(gcsURI, "gs://") {
		fmt.Printf("Error: Invalid GCS link format: %s\n", gcsURI)
		return "", false
	}
	parts := strings.SplitN(gcsURI[5:], "/", 3)
	if len(parts) < 3 {
		fmt.Printf("Error: Failed to parse GCS link '%s': GCS link must contain a bucket, prefix, and relative_path.\n", gcsURI)
		return "", false
	}
	prefix, relativePath := parts[1], parts[2]

	if strings.HasSuffix(relativePath, ".html") {
		relativePath = strings.TrimSuffix(relativePath, ".html") + ".md"
	}
	if strings.HasSuffix(relativePath, ".txt") {
		relativePath = strings.TrimSuffix(relativePath, ".txt") + ".yaml"
	}

	if prefix == "adk-docs" && strings.HasPrefix(relativePath, "docs/") {
		pathAfterDocs := relativePath[len("docs/"):]
		if !strings.HasSuffix(pathAfterDocs, ".md") {
			return generateGitHubURL(prefix, relativePath), true
		}
		const baseURL = "https://google.github.io/adk-docs/"
		var finalSegment string
		if path.Base(pathAfterDocs) == "index.md" {
			// os.path.dirname: "" for a bare file name.
			if i := strings.LastIndex(pathAfterDocs, "/"); i >= 0 {
				finalSegment = pathAfterDocs[:i]
			}
		} else {
			finalSegment = strings.TrimSuffix(pathAfterDocs, ".md")
		}
		if finalSegment != "" && !strings.HasSuffix(finalSegment, "/") {
			finalSegment += "/"
		}
		base, _ := url.Parse(baseURL)
		ref, err := url.Parse(finalSegment)
		potential := baseURL
		if err == nil {
			potential = base.ResolveReference(ref).String()
		}
		if checkURLExists(potential) {
			return potential, true
		}
		return generateGitHubURL(prefix, relativePath), true
	}
	return generateGitHubURL(prefix, relativePath), true
}

// tools.py

type discussionNumberArgs struct {
	DiscussionNumber int `json:"discussion_number"`
}

func getDiscussionAndComments(_ agent.Context, in discussionNumberArgs) (map[string]any, error) {
	fmt.Printf("Attempting to get discussion #%d and its comments\n", in.DiscussionNumber)
	query := `
        query($owner: String!, $repo: String!, $discussionNumber: Int!) {
          repository(owner: $owner, name: $repo) {
            discussion(number: $discussionNumber) {
              id
              title
              body
              createdAt
              closed
              author {
                login
              }
              # For each discussion, fetch the latest 20 labels.
              labels(last: 20) {
                nodes {
                  id
                  name
                }
              }
              # For each discussion, fetch the latest 100 comments.
              comments(last: 100) {
                nodes {
                  id
                  body
                  createdAt
                  author {
                    login
                  }
                  # For each discussion, fetch the latest 50 replies
                  replies(last: 50) {
                    nodes {
                      id
                      body
                      createdAt
                      author {
                        login
                      }
                    }
                  }
                }
              }
            }
          }
        }
    `
	variables := map[string]any{"owner": owner, "repo": repo, "discussionNumber": in.DiscussionNumber}
	response, err := runGraphQLQuery(query, variables)
	if err != nil {
		return errorResponse(err.Error()), nil
	}
	if errs, ok := response["errors"]; ok {
		return errorResponse(team.Repr(errs)), nil
	}
	discussion := team.Get(response, "data", "repository", "discussion")
	if discussion == nil {
		return errorResponse(fmt.Sprintf("Discussion #%d not found.", in.DiscussionNumber)), nil
	}
	return map[string]any{"status": "success", "discussion": discussion}, nil
}

type addCommentArgs struct {
	DiscussionID string `json:"discussion_id"`
	CommentBody  string `json:"comment_body"`
}

func addCommentToDiscussion(_ agent.Context, in addCommentArgs) (map[string]any, error) {
	fmt.Printf("Adding comment to discussion %s\n", in.DiscussionID)
	query := `
        mutation($discussionId: ID!, $body: String!) {
          addDiscussionComment(input: {discussionId: $discussionId, body: $body}) {
            comment {
              id
              body
              createdAt
              author {
                login
              }
            }
          }
        }
    `
	body := in.CommentBody
	if !strings.HasPrefix(body, "**Response from ADK Answering Agent") {
		body = "**Response from ADK Answering Agent (experimental, answer may be" +
			" inaccurate)**\n\n" + body
	}
	response, err := runGraphQLQuery(query, map[string]any{"discussionId": in.DiscussionID, "body": body})
	if err != nil {
		return errorResponse(err.Error()), nil
	}
	if errs, ok := response["errors"]; ok {
		return errorResponse(team.Repr(errs)), nil
	}
	return map[string]any{"status": "success", "comment": team.Get(response, "data", "addDiscussionComment", "comment")}, nil
}

// getLabelID is get_label_id; "" where Python returns None.
func getLabelID(labelName string) string {
	fmt.Printf("Finding ID for label '%s'...\n", labelName)
	query := `
    query($owner: String!, $repo: String!, $labelName: String!) {
      repository(owner: $owner, name: $repo) {
        label(name: $labelName) {
          id
        }
      }
    }
    `
	response, err := runGraphQLQuery(query, map[string]any{"owner": owner, "repo": repo, "labelName": labelName})
	if err != nil {
		fmt.Printf("[Warning] Error from GitHub API: %v\n", err)
		return ""
	}
	if errs, ok := response["errors"]; ok {
		fmt.Printf("[Warning] Error from GitHub API response for label '%s': %s\n", labelName, team.Repr(errs))
		return ""
	}
	if info := team.Map(team.Get(response, "data", "repository", "label")); len(info) > 0 {
		return team.Str(info["id"])
	}
	fmt.Printf("[Warning] Label information for '%s' not found.\n", labelName)
	return ""
}

type addLabelArgs struct {
	DiscussionID string `json:"discussion_id"`
	LabelName    string `json:"label_name"`
}

func addLabelToDiscussion(_ agent.Context, in addLabelArgs) (map[string]any, error) {
	fmt.Printf("Attempting to add label '%s' to discussion %s...\n", in.LabelName, in.DiscussionID)
	labelID := getLabelID(in.LabelName)
	if labelID == "" {
		return errorResponse(fmt.Sprintf("Label '%s' not found.", in.LabelName)), nil
	}
	mutation := `
    mutation AddLabel($discussionId: ID!, $labelId: ID!) {
      addLabelsToLabelable(input: {labelableId: $discussionId, labelIds: [$labelId]}) {
        clientMutationId
      }
    }
    `
	response, err := runGraphQLQuery(mutation, map[string]any{"discussionId": in.DiscussionID, "labelId": labelID})
	if err != nil {
		return errorResponse(err.Error()), nil
	}
	if errs, ok := response["errors"]; ok {
		return errorResponse(team.Repr(errs)), nil
	}
	return map[string]any{"status": "success", "label_id": labelID, "label_name": in.LabelName}, nil
}

type gcsArgs struct {
	GCSURIs []string `json:"gcs_uris"`
}

func convertGCSLinksToHTTPS(_ agent.Context, in gcsArgs) (map[string]any, error) {
	out := map[string]any{}
	for _, u := range in.GCSURIs {
		if converted, ok := convertGCSToHTTPS(u); ok {
			out[u] = converted
		} else {
			out[u] = nil
		}
	}
	return out, nil
}

// listMostRecentDiscussions is main.py's list_most_recent_discussions; nil
// where Python returns None.
func listMostRecentDiscussions(count int) []int {
	fmt.Printf("Attempting to fetch the %d most recently updated discussions from %s/%s...\n", count, owner, repo)
	query := `
    query($owner: String!, $repo: String!, $count: Int!) {
      repository(owner: $owner, name: $repo) {
        discussions(
          first: $count
          orderBy: {field: UPDATED_AT, direction: DESC}
        ) {
          nodes {
            title
            number
            updatedAt
            author {
              login
            }
          }
        }
      }
    }
    `
	response, err := runGraphQLQuery(query, map[string]any{"owner": owner, "repo": repo, "count": count})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Request failed: %v\n", err)
		return nil
	}
	if errs, ok := response["errors"]; ok {
		fmt.Fprintf(os.Stderr, "Error from GitHub API: %s\n", team.Repr(errs))
		return nil
	}
	numbers := []int{}
	for _, d := range team.List(team.Get(response, "data", "repository", "discussions", "nodes")) {
		numbers = append(numbers, team.Int(team.Get(d, "number")))
	}
	return numbers
}
