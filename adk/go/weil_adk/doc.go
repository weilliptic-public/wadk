// Package weiladk gives Google ADK (Go, v2) agents a WeilChain wallet
// identity, so they can call any applet method on chain as themselves.
//
// You write ADK as you already do; each weiladk entry point is a twin of an
// ADK call — the same config, plus the identity:
//
//	llmagent.New(cfg)             → weiladk.NewLLMAgent(id, cfg)   (or weiladk.Bind(agent, id))
//	plugin.New(cfg)               → weiladk.NewPlugin(cfg, opts...)
//	remoteagent.NewA2A(cfg)       → weiladk.NewA2A(id, cfg, opts...)
//	adka2a.NewExecutor(cfg)       → weiladk.NewExecutor(cfg, opts...)
//	a2asrv.NewJSONRPCHandler(h)   → weiladk.SecureA2A(h, agent, weiladk.WithService("engg.weil"))
//	functiontool.New(cfg, fn)     → functiontool.New(cfg, weiladk.SecuredUser("engg.weil", fn))
//
// The A2A and secured layers are wire-compatible with the Python SDK
// (weil_ai), so Go and Python agents call and gate each other. The Python
// SDK's names map to Go as:
//
//	Python (weil_ai, weil_wallet)               Go
//	@weil_ai.agent(identity=...)                weiladk.FromIdentity, weiladk.Bind
//	agent.setup_secure_a2a(service=...)         weiladk.NewPlugin(cfg, weiladk.WithSecureA2A(...)), weiladk.SecureA2A(..., weiladk.WithService(...))
//	agent.to_secure_a2a()                       weiladk.ToSecureA2A, weiladk.NewExecutor
//	@secured_user / @secured_admin              weiladk.SecuredUser / weiladk.SecuredAdmin, secured.User / secured.Admin
//	current_wallet_addr()                       weiladk.CallerFromContext, secured.WalletFromContext, weilmcp.Wallet
//	WalletNotPermittedError                     *secured.NotPermittedError
//	SecuredExprError                            secured.ErrBadRule
//	SecuredCheckError                           *secured.CheckError
//	flush_audits()                              weiladk.FlushAudits
//	make_adk_pensieve_tools                     weiladk.PensieveTools
//	AgentTool(include_plugins=True)             weiladk.NewAgentTool
//	client.record_agent_token_usage             (*client.WeilClient).RecordAgentTokenUsage
//
// Inside any ADK callback or tool, Invoke (blocking) and Submit (queued)
// call applets as the agent that is acting:
//
//	id, _ := weiladk.FromWalletFile("agent.wc")
//	assistant, _ := weiladk.NewLLMAgent(id, llmagent.Config{Name: "assistant", Model: llm, Tools: tools})
//
//	p, _ := weiladk.NewPlugin(plugin.Config{
//		BeforeToolCallback: func(ctx agent.Context, t tool.Tool, args map[string]any) (map[string]any, error) {
//			res, err := weiladk.Invoke(ctx, weiladk.Call{
//				Applet: weiladk.AppletName("budget::acme"), Method: "authorize",
//				Args:   map[string]any{"tool": t.Name(), "args": args},
//			})
//			// ... decode res; return an error to veto the tool
//			return nil, err
//		},
//	}, weiladk.WithActions(weiladk.AuditAction(weiladk.AuditConfig{})))
//
//	r, _ := runner.New(runner.Config{
//		AppName: "support", Agent: assistant, SessionService: session.InMemoryService(),
//		PluginConfig: runner.PluginConfig{Plugins: []*plugin.Plugin{p}},
//	})
//
// Applets are named by contract address (AppletID), service name
// (AppletName, e.g. "counter::acme"), or systemic name (Systemic("auditor")
// is "auditor::weil"). Actions offer a declarative alternative to callbacks,
// with Blocking or NonBlocking modes and failure policies; the auditor is
// one preset (AuditAction).
package weiladk
