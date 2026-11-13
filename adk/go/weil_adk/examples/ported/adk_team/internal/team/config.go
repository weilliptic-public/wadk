// Package team is the shared Weil configuration and helpers of the ported
// ADK team agents — the Go side of Python's adk_team/weil_config.py, plus
// what the Python agents get from google.adk, requests and dotenv.
//
// Environment (as weil_config.py):
//
//	WEIL_ADK_TEAM_AGENT_IDENTITY  Agent Registry identity every team agent acts
//	                              under (required unless WEIL_ADK_TEAM_WALLET)
//	WEIL_ADK_TEAM_WALLET          alternatively, a wallet.wc file
//	WEIL_ADK_TEAM_SERVICE         scope of every secured tool (default engg.google)
//	WEIL_ADK_TEAM_MODEL           LiteLLM-style "provider/model" (default openai/gpt-5.1)
//
// Sentinel is reached as in the other examples (WEIL_SENTINEL_HOST, default
// the dev platform; see internal/devnet). Python's verify=False
// applies to a local platform only: elsewhere Sentinel's certificate is
// verified.
package team

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"sync"

	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/model/gemini"
	"google.golang.org/adk/v2/model/openaimodel"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/claudemodel"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

// DefaultService and DefaultModel are weil_config.py's defaults.
const (
	DefaultService = "engg.google"
	DefaultModel   = "openai/gpt-5.1"
)

// Getenv is os.getenv(key, def): def when key is unset.
func Getenv(key, def string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return def
}

// MustGetenv returns key's value or exits with Python's ValueError text when
// it is unset or empty.
func MustGetenv(key string) string {
	v := os.Getenv(key)
	if v == "" {
		Fatalf("ValueError: %s environment variable not set", key)
	}
	return v
}

// IsTrue is os.getenv(key, def).lower() in ["true", "1"].
func IsTrue(key, def string) bool {
	v := strings.ToLower(Getenv(key, def))
	return v == "true" || v == "1"
}

// Service is WEIL_ADK_TEAM_SERVICE, the scope every team tool is secured
// under.
func Service() string { return Getenv("WEIL_ADK_TEAM_SERVICE", DefaultService) }

// ModelName is WEIL_ADK_TEAM_MODEL.
func ModelName() string { return Getenv("WEIL_ADK_TEAM_MODEL", DefaultModel) }

// Model is LiteLlm(model=WEIL_ADK_TEAM_MODEL): a LiteLLM-style
// "provider/model" name mapped onto ADK-Go's model packages, with each
// provider's LiteLLM key variables.
//
//	openai/<m>     openaimodel (Chat Completions, as LiteLLM), OPENAI_API_KEY,
//	               OPENAI_API_BASE or OPENAI_BASE_URL
//	anthropic/<m>  claudemodel, ANTHROPIC_API_KEY (or API_KEY)
//	gemini/<m>     gemini, GEMINI_API_KEY (or GOOGLE_API_KEY)
//
// A name without a provider is routed as LiteLLM does: claude-* to
// Anthropic, gemini-* to Gemini, anything else to OpenAI.
func Model(ctx context.Context) model.LLM {
	m, err := newModel(ctx, ModelName())
	if err != nil {
		Fatalf("model %q: %v", ModelName(), err)
	}
	return m
}

func newModel(ctx context.Context, name string) (model.LLM, error) {
	provider, id, ok := strings.Cut(name, "/")
	if !ok {
		id = name
		switch {
		case strings.HasPrefix(name, "claude"):
			provider = "anthropic"
		case strings.HasPrefix(name, "gemini"):
			provider = "gemini"
		default:
			provider = "openai"
		}
	}
	switch provider {
	case "openai":
		base := os.Getenv("OPENAI_API_BASE")
		if base == "" {
			base = os.Getenv("OPENAI_BASE_URL")
		}
		return openaimodel.NewModel(ctx, id, &openaimodel.ClientConfig{
			APIKey:  os.Getenv("OPENAI_API_KEY"),
			BaseURL: base,
			API:     openaimodel.APIChatCompletions,
		})
	case "anthropic":
		key := os.Getenv("ANTHROPIC_API_KEY")
		if key == "" {
			key = os.Getenv("API_KEY")
		}
		return claudemodel.NewModel(ctx, id, &claudemodel.Config{APIKey: key})
	case "gemini":
		key := os.Getenv("GEMINI_API_KEY")
		if key == "" {
			key = os.Getenv("GOOGLE_API_KEY")
		}
		return gemini.NewModel(ctx, id, &genai.ClientConfig{APIKey: key})
	}
	return nil, fmt.Errorf("unsupported provider %q", provider)
}

var (
	identityOnce sync.Once
	identity     *weiladk.Identity
)

// Identity is the one identity every team agent acts under
// (@weil_ai.agent(identity=WEIL_ADK_TEAM_AGENT_IDENTITY, verify=False)),
// loaded once from WEIL_ADK_TEAM_AGENT_IDENTITY, or from the wallet file
// WEIL_ADK_TEAM_WALLET.
func Identity(ctx context.Context) *weiladk.Identity {
	identityOnce.Do(func() {
		var err error
		switch {
		case os.Getenv("WEIL_ADK_TEAM_WALLET") != "":
			identity, err = weiladk.FromWalletFile(os.Getenv("WEIL_ADK_TEAM_WALLET"), devnet.ClientOptions()...)
		case os.Getenv("WEIL_ADK_TEAM_AGENT_IDENTITY") != "":
			identity, err = weiladk.FromIdentity(ctx, os.Getenv("WEIL_ADK_TEAM_AGENT_IDENTITY"), devnet.ClientOptions()...)
		default:
			err = errors.New("set WEIL_ADK_TEAM_AGENT_IDENTITY (or WEIL_ADK_TEAM_WALLET)")
		}
		if err != nil {
			Fatalf("team identity: %v", err)
		}
	})
	return identity
}

// LoadDotEnv is load_dotenv(path, override=True): KEY=VALUE lines of path,
// if it exists, override the environment. An empty path means ".env".
func LoadDotEnv(path string) {
	if path == "" {
		path = ".env"
	}
	f, err := os.Open(path)
	if err != nil {
		return
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		line = strings.TrimPrefix(line, "export ")
		k, v, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		k, v = strings.TrimSpace(k), strings.TrimSpace(v)
		if len(v) >= 2 && (v[0] == '"' || v[0] == '\'') && v[len(v)-1] == v[0] {
			if v[0] == '"' {
				if u, err := strconv.Unquote(v); err == nil {
					v = u
				} else {
					v = v[1 : len(v)-1]
				}
			} else {
				v = v[1 : len(v)-1]
			}
		} else if i := strings.Index(v, " #"); i >= 0 {
			v = strings.TrimSpace(v[:i])
		}
		os.Setenv(k, v)
	}
}

// ParseNumberString is the agents' parse_number_string: def for an empty
// string, else the integer, else a warning on stderr and def.
func ParseNumberString(s string, def int) int {
	if s == "" {
		return def
	}
	n, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: Invalid number string: %s. Defaulting to %d.\n", s, def)
		return def
	}
	return n
}

// ReadFile is the agents' read_file: the file's content, or "" after
// printing that it is missing.
func ReadFile(path string) string {
	b, err := os.ReadFile(path)
	if err != nil {
		fmt.Printf("Error: File not found: %s.\n", path)
		return ""
	}
	return string(b)
}

// LookupEnv is os.LookupEnv.
func LookupEnv(key string) (string, bool) { return os.LookupEnv(key) }
