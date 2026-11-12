// dice-prime is the Go version of the Python SDK's hello_world_ma example
// (python/examples/adk-ported/multi_agent/hello_world_ma): a root agent that
// hands dice rolls to roll_agent and prime checks to prime_agent, served over
// secured A2A on 127.0.0.1:8001 like the Python a2a_app.
//
// Each agent has its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent        identity    <KEY>       grant
//	root_agent   dice-desk   DICE_DESK   - (callers need finance.acme)
//	roll_agent   roll-desk   ROLL_DESK   finance.acme (roll_die)
//	prime_agent  prime-desk  PRIME_DESK  infra.acme (check_prime)
//
// A secured tool authorizes as the agent running it, so each sub-agent's own
// wallet decides its tool, and its audit records name dice-desk as the caller
// that delegated to it.
//
//   - roll_die is SecuredUser("finance.acme"): it runs once roll-desk holds
//     Execution or Management in finance.acme.
//   - check_prime is SecuredUser("infra.acme"): it runs once prime-desk holds
//     one in infra.acme.
//   - The A2A server serves only callers holding Execution or Management in
//     finance.acme (Python: setup_secure_a2a(service="finance.acme")).
//
// dice-desk's own permissions do not carry over to its sub-agents. Grant
// them with:
//
//	set_permissions -n acme -g finance -p Execution -s '["<roll-desk address>"]'
//	set_permissions -n acme -g infra -p Execution -s '["<prime-desk address>"]'
//
// The model is Claude (Python uses LiteLlm openai/gpt-4o).
package main

import (
	"context"
	"fmt"
	"log"
	"math/rand/v2"
	"net"
	"net/http"
	"slices"
	"strings"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/exampletool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

const (
	listenAddr = "127.0.0.1:8001" // Python: to_secure_a2a(host="127.0.0.1", port=8001)
	service    = "finance.acme"   // Python: setup_secure_a2a(service="finance.acme")
)

// ── roll_agent ───────────────────────────────────────────────────────────

type RollInput struct {
	Sides int `json:"sides"`
}

// rollDie is gated by weiladk.SecuredUser("finance.acme") in main. Like the
// Python tool it returns a bare int, which reaches the model as
// {"result": n}.
func rollDie(_ agent.Context, in RollInput) (int, error) {
	if in.Sides < 1 { // Python's random.randint(1, sides) raises here
		return 0, fmt.Errorf("empty range in randint(1, %d)", in.Sides)
	}
	return rand.IntN(in.Sides) + 1, nil
}

// ── prime_agent ──────────────────────────────────────────────────────────

type PrimeInput struct {
	Nums []int `json:"nums"`
}

// checkPrime is gated by weiladk.SecuredUser("infra.acme") in main.
func checkPrime(_ agent.Context, in PrimeInput) (string, error) {
	var primes []int
	for _, n := range in.Nums {
		if isPrime(n) && !slices.Contains(primes, n) { // Python collects a set
			primes = append(primes, n)
		}
	}
	if len(primes) == 0 {
		return "No prime numbers found.", nil
	}
	slices.Sort(primes) // CPython iterates a set of small ints in ascending order
	strs := make([]string, len(primes))
	for i, n := range primes {
		strs[i] = fmt.Sprint(n)
	}
	return strings.Join(strs, ", ") + " are prime numbers.", nil
}

func isPrime(n int) bool {
	if n <= 1 {
		return false
	}
	for i := 2; i*i <= n; i++ {
		if n%i == 0 {
			return false
		}
	}
	return true
}

// generateContentConfig turns off the DANGEROUS_CONTENT filter on every
// agent, to avoid false alarms about rolling dice.
func generateContentConfig() *genai.GenerateContentConfig {
	return &genai.GenerateContentConfig{
		SafetySettings: []*genai.SafetySetting{{
			Category:  genai.HarmCategoryDangerousContent,
			Threshold: genai.HarmBlockThresholdOff,
		}},
	}
}

func modelContent(text string) *genai.Content {
	return genai.NewContentFromText(text, genai.RoleModel)
}

func main() {
	ctx := context.Background()
	deskID := agentIdentity(ctx, "dice-desk")
	rollID := agentIdentity(ctx, "roll-desk")
	primeID := agentIdentity(ctx, "prime-desk")

	llm := devnet.Claude() // API_KEY, else ANTHROPIC_API_KEY; CLAUDE_MODEL

	rollTool, err := functiontool.New(
		functiontool.Config{Name: "roll_die", Description: "Roll a die and return the rolled result."},
		weiladk.SecuredUser("finance.acme", rollDie))
	if err != nil {
		log.Fatalf("create roll_die: %v", err)
	}
	primeTool, err := functiontool.New(
		functiontool.Config{Name: "check_prime", Description: "Check if a given list of numbers are prime."},
		weiladk.SecuredUser("infra.acme", checkPrime))
	if err != nil {
		log.Fatalf("create check_prime: %v", err)
	}

	rollAgent, err := weiladk.NewLLMAgent(rollID, llmagent.Config{
		Name:        "roll_agent",
		Mode:        llmagent.ModeSingleTurn,
		Model:       llm,
		Description: "Handles rolling dice of different sizes.",
		Instruction: `
        You are responsible for rolling dice based on the user's request.
        When asked to roll a die, you must call the roll_die tool with the number of sides as an integer.
        `,
		Tools:                 []tool.Tool{rollTool},
		GenerateContentConfig: generateContentConfig(),
	})
	if err != nil {
		log.Fatalf("create roll_agent: %v", err)
	}
	primeAgent, err := weiladk.NewLLMAgent(primeID, llmagent.Config{
		Name:        "prime_agent",
		Mode:        llmagent.ModeSingleTurn,
		Model:       llm,
		Description: "Handles checking if numbers are prime.",
		Instruction: `
        You are responsible for checking whether numbers are prime.
        When asked to check primes, you must call the check_prime tool with a list of integers.
        Never attempt to determine prime numbers manually.
        Return the prime number results to the root agent.
        `,
		Tools:                 []tool.Tool{primeTool},
		GenerateContentConfig: generateContentConfig(),
	})
	if err != nil {
		log.Fatalf("create prime_agent: %v", err)
	}

	examples, err := exampletool.New(exampletool.ExampleToolConfig{
		Examples: []*exampletool.Example{
			{
				Input:  genai.NewContentFromText("Roll a 6-sided die.", genai.RoleUser),
				Output: []*genai.Content{modelContent("I rolled a 4 for you.")},
			},
			{
				Input:  genai.NewContentFromText("Is 7 a prime number?", genai.RoleUser),
				Output: []*genai.Content{modelContent("Yes, 7 is a prime number.")},
			},
			{
				Input: genai.NewContentFromText("Roll a 10-sided die and check if it's prime.", genai.RoleUser),
				Output: []*genai.Content{
					modelContent("I rolled an 8 for you."),
					modelContent("8 is not a prime number."),
				},
			},
		},
	})
	if err != nil {
		log.Fatalf("create example tool: %v", err)
	}

	rootAgent, err := weiladk.NewLLMAgent(deskID, llmagent.Config{
		Name:  "root_agent",
		Model: llm,
		Instruction: `
        You are a helpful assistant that can roll dice and check if numbers are prime.
        You delegate rolling dice tasks to the roll_agent and prime checking tasks to the prime_agent.
        Follow these steps:
        1. If the user asks to roll a die, delegate to the roll_agent.
        2. If the user asks to check primes, delegate to the prime_agent.
        3. If the user asks to roll a die and then check if the result is prime, call roll_agent first, then pass the result to prime_agent.
        Always clarify the results before proceeding.
        `,
		GlobalInstruction: "You are DicePrimeBot, ready to roll dice and check prime numbers.",
		SubAgents:         []agent.Agent{rollAgent, primeAgent},
		// Python's setup_secure_a2a adds the Pensieve tools to the root agent.
		Tools:                 append([]tool.Tool{examples}, weiladk.PensieveTools()...),
		GenerateContentConfig: generateContentConfig(),
	})
	if err != nil {
		log.Fatalf("create root_agent: %v", err)
	}

	serve(rootAgent)
}

// serve exposes the agent over A2A on listenAddr, as Python's
// to_secure_a2a(host="127.0.0.1", port=8001) does. ToSecureA2A builds
// to_a2a's card, installs the agent and secured-tool audits and the run
// traces (setup_secure_a2a), and verifies each caller and gates it on
// finance.acme.
func serve(served agent.Agent) {
	handler, err := weiladk.ToSecureA2A(served, "http://"+listenAddr, weiladk.WithService(service))
	if err != nil {
		log.Fatalf("secure A2A handler: %v", err)
	}
	listener, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("listen %s: %v", listenAddr, err)
	}
	log.Printf("%s serving secured A2A on http://%s", served.Name(), listenAddr)
	log.Fatal(http.Serve(listener, handler))
}

// agentIdentity returns the identity of the agent called name (see
// devnet.Agent): WEIL_IDENTITY_<KEY>, WEIL_WALLET_<KEY>, else
// ~/.weilliptic/agents/<name>.wc. Sentinel is WEIL_SENTINEL_HOST (default: the
// dev platform); its TLS certificate is verified except for a local platform.
func agentIdentity(ctx context.Context, name string) *weiladk.Identity {
	id, err := devnet.Agent(ctx, name)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("%s wallet: %s", name, id.Address())
	return id
}
