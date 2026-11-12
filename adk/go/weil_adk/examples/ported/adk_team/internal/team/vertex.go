package team

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"
	"sync"

	"golang.org/x/oauth2"
	"golang.org/x/oauth2/google"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/geminitool"
	"google.golang.org/genai"
)

const cloudPlatformScope = "https://www.googleapis.com/auth/cloud-platform"

// VertexAISearch is VertexAiSearchTool(data_store_id=id) on its own: Gemini's
// built-in Vertex AI Search retrieval, which only Gemini models run.
func VertexAISearch(dataStoreID string) tool.Tool {
	return geminitool.New("vertex_ai_search", "Vertex AI Search", &genai.Tool{
		Retrieval: &genai.Retrieval{VertexAISearch: &genai.VertexAISearch{Datastore: dataStoreID}},
	})
}

// SearchAPIError is a Discovery Engine error, formatted as Python's
// GoogleAPICallError ("<code> <message>").
type SearchAPIError struct {
	Code    int
	Message string
}

func (e *SearchAPIError) Error() string { return fmt.Sprintf("%d %s", e.Code, e.Message) }

// DiscoverySearch posts a SearchRequest to servingConfig's :search with
// tokens from ts and returns the decoded SearchResponse.
func DiscoverySearch(ctx context.Context, ts oauth2.TokenSource, host, quotaProject, servingConfig string, req map[string]any) (map[string]any, error) {
	body, err := json.Marshal(req)
	if err != nil {
		return nil, err
	}
	u := fmt.Sprintf("https://%s/v1beta/%s:search", host, servingConfig)
	hreq, err := http.NewRequestWithContext(ctx, http.MethodPost, u, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	hreq.Header.Set("Content-Type", "application/json")
	if quotaProject != "" {
		hreq.Header.Set("x-goog-user-project", quotaProject)
	}
	resp, err := oauth2.NewClient(ctx, ts).Do(hreq)
	if err != nil {
		return nil, &SearchAPIError{Code: 503, Message: err.Error()}
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	var out map[string]any
	_ = json.Unmarshal(data, &out)
	if resp.StatusCode >= 400 {
		msg := Str(Get(out, "error", "message"))
		if msg == "" {
			msg = strings.TrimSpace(string(data))
		}
		return nil, &SearchAPIError{Code: resp.StatusCode, Message: msg}
	}
	if out == nil {
		out = map[string]any{}
	}
	return out, nil
}

var (
	structuredStoreErr = regexp.MustCompile(`(?i)search_result_mode.*DOCUMENTS`)
	locationRE         = regexp.MustCompile(`(?i)/locations/([a-z0-9-]+)(?:/|$)`)
)

// DiscoveryEngineSearch is the tool Python's ADK puts in place of
// VertexAiSearchTool(data_store_id=id, bypass_multi_tools_limit=True) when
// an agent has other tools: DiscoveryEngineSearchTool, a function tool named
// discovery_engine_search over the Discovery Engine search API with
// Application Default Credentials. It tries CHUNKS results first and falls
// back to DOCUMENTS for a structured data store, once per tool.
func DiscoveryEngineSearch(dataStoreID string) tool.Tool {
	type args struct {
		Query string `json:"query"`
	}
	host := "discoveryengine.googleapis.com"
	if m := locationRE.FindStringSubmatch(dataStoreID); m != nil && strings.ToLower(m[1]) != "global" {
		host = strings.ToLower(m[1]) + "-" + host
	}
	servingConfig := dataStoreID + "/servingConfigs/default_config"
	var (
		mu       sync.Mutex
		mode     string
		ts       oauth2.TokenSource
		quota    string
		tokenErr error
		tsOnce   sync.Once
	)
	search := func(ctx context.Context, query, mode string) (map[string]any, error) {
		spec := map[string]any{"searchResultMode": mode}
		if mode == "CHUNKS" {
			spec["chunkSpec"] = map[string]any{"numPreviousChunks": 0, "numNextChunks": 0}
		}
		resp, err := DiscoverySearch(ctx, ts, host, quota, servingConfig, map[string]any{"query": query, "contentSearchSpec": spec})
		if err != nil {
			return nil, err
		}
		results := []any{}
		for _, item := range List(resp["results"]) {
			if mode == "DOCUMENTS" {
				if doc := Map(Get(item, "document")); doc != nil {
					results = append(results, parseDocument(doc))
				}
			} else if chunk := Map(Get(item, "chunk")); chunk != nil {
				results = append(results, parseChunk(chunk))
			}
		}
		return map[string]any{"status": "success", "results": results}, nil
	}
	return Tool("discovery_engine_search", `Search through Vertex AI Search's discovery engine search API.

Args:
  query: The search query.

Returns:
  A dictionary containing the status of the request and the list of
  search results, which contains the title, url and content.`,
		func(ctx agent.Context, in args) (map[string]any, error) {
			tsOnce.Do(func() {
				creds, err := google.FindDefaultCredentials(ctx, cloudPlatformScope)
				if err != nil {
					tokenErr = err
					return
				}
				ts = creds.TokenSource
				var meta struct {
					QuotaProjectID string `json:"quota_project_id"`
				}
				_ = json.Unmarshal(creds.JSON, &meta)
				quota = meta.QuotaProjectID
			})
			if tokenErr != nil {
				return nil, tokenErr
			}
			result, err := func() (map[string]any, error) {
				mu.Lock()
				m := mode
				if m != "" {
					mu.Unlock()
					return search(ctx, in.Query, m)
				}
				defer mu.Unlock()
				r, err := search(ctx, in.Query, "CHUNKS")
				if err == nil {
					mode = "CHUNKS"
					return r, nil
				}
				if !structuredStoreErr.MatchString(err.Error()) {
					return nil, err
				}
				mode = "DOCUMENTS"
				return search(ctx, in.Query, "DOCUMENTS")
			}()
			if _, ok := err.(*SearchAPIError); ok {
				return map[string]any{"status": "error", "error_message": err.Error()}, nil
			}
			return result, err
		})
}

func parseChunk(chunk map[string]any) map[string]any {
	title, uri := "", ""
	if meta := Map(chunk["documentMetadata"]); meta != nil {
		title, uri = Str(meta["title"]), Str(meta["uri"])
		if sd := Map(meta["structData"]); sd != nil {
			if u, ok := sd["uri"]; ok {
				uri = Str(u)
			}
		}
	}
	return map[string]any{"title": title, "url": uri, "content": Str(chunk["content"])}
}

func parseDocument(doc map[string]any) map[string]any {
	title, uri, content := "", "", ""
	if sd := Map(doc["structData"]); len(sd) > 0 {
		data := map[string]any{}
		for k, v := range sd {
			data[k] = v
		}
		title = Str(data["title"])
		delete(data, "title")
		if v, ok := data["uri"]; ok {
			uri = Str(v)
			delete(data, "uri")
		} else {
			uri = Str(data["link"])
			delete(data, "link")
		}
		b, _ := json.Marshal(data)
		content = string(b)
	} else if dd := Map(doc["derivedStructData"]); len(dd) > 0 {
		title, uri = Str(dd["title"]), Str(dd["link"])
		var texts []string
		for _, s := range List(dd["snippets"]) {
			if sn := Str(Get(s, "snippet")); sn != "" {
				texts = append(texts, sn)
			} else {
				texts = append(texts, Repr(s))
			}
		}
		content = strings.Join(texts, "\n")
		if content == "" {
			var answers []string
			for _, a := range List(dd["extractive_answers"]) {
				answers = append(answers, Repr(a))
			}
			content = strings.Join(answers, "\n")
		}
	}
	return map[string]any{"title": title, "url": uri, "content": content}
}
