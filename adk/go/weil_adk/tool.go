package weiladk

import (
	"errors"
	"fmt"
	"net/http"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
)

// AppletToolConfig configures NewAppletTool.
type AppletToolConfig struct {
	// Name and Description are what the model sees.
	Name        string
	Description string
	Applet      Applet
	Method      string
	// InputSchema describes the method's argument object to the model.
	// Without it the model is told only that the tool takes an object.
	InputSchema *jsonschema.Schema
	// Identity, if set, picks the wallet that signs the call. By default
	// the call is signed by the calling agent's bound identity (see Bind).
	Identity      IdentityResolver
	HideArgs      bool
	FireAndForget bool
	// RequireConfirmation asks the user before every call (ADK
	// human-in-the-loop), e.g. for methods that move funds.
	RequireConfirmation bool
}

// NewAppletTool exposes one applet method to the model as a tool. The model's
// arguments become the method's argument object, the call is signed by the
// calling agent's identity, and the method's decoded return value comes back
// as {"result": ...}. Applet errors reach the model as tool errors.
func NewAppletTool(cfg AppletToolConfig) (tool.Tool, error) {
	if cfg.Name == "" || cfg.Method == "" || cfg.Applet.IsZero() {
		return nil, errors.New("weiladk: applet tool needs Name, Applet and Method")
	}
	if cfg.Description == "" {
		cfg.Description = fmt.Sprintf("Calls %s on applet %s.", cfg.Method, cfg.Applet)
	}

	handler := func(ctx agent.Context, args map[string]any) (map[string]any, error) {
		id := IdentityOf(ctx)
		if cfg.Identity != nil {
			ev := eventFromAgentContext(HookBeforeTool, ctx)
			ev.ToolArgs, ev.FunctionCallID = args, ctx.FunctionCallID()
			var err error
			if id, err = cfg.Identity(ev); err != nil {
				return nil, err
			}
		}
		if id == nil {
			return nil, ErrNoIdentity
		}

		res, err := id.Invoke(ctx, Call{
			Applet:        cfg.Applet,
			Method:        cfg.Method,
			Args:          args,
			HideArgs:      cfg.HideArgs,
			FireAndForget: cfg.FireAndForget,
		})
		if err != nil {
			return nil, err
		}
		if cfg.FireAndForget {
			return map[string]any{"status": "submitted"}, nil
		}

		var out any
		if err := res.Decode(&out); err != nil {
			return nil, err
		}
		return map[string]any{"result": out}, nil
	}

	return functiontool.New(functiontool.Config{
		Name:                cfg.Name,
		Description:         cfg.Description,
		InputSchema:         cfg.InputSchema,
		RequireConfirmation: cfg.RequireConfirmation,
	}, handler)
}

// AuthTransport returns an http.RoundTripper that signs every request with
// id's wallet, bound to the request body (see client.AuthTransport). base
// nil means http.DefaultTransport.
func AuthTransport(id *Identity, base http.RoundTripper) http.RoundTripper {
	return client.NewAuthTransport(id.client, base)
}
