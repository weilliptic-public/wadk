// Package componentowners is adk_team/component_owners.py: the
// component-owner map of the PR triaging agent.
//
// One of a component's owners becomes the PR assignee (its shepherd), taking
// turns by PR number when a component has several.
//
// github login != corp ldap, so these are the login form. Keep each
// component's first owner in sync with the OWNERS file, which is the
// authority.
package componentowners

// LabelToOwner maps a component label to the GitHub logins of the owners who
// shepherd that component.
var LabelToOwner = map[string][]string{
	"a2a":           {"mykytanetipa"},
	"agent engine":  {"yeesian"},
	"auth":          {"xuanyang15"},
	"bq":            {"shobsi"},
	"cli":           {"wyf7107"},
	"core":          {"DeanChensj"},
	"documentation": {"joefernandez"},
	"eval":          {"i-yliu"},
	"integrations":  {"wukath"},
	"live":          {"wuliang229"},
	"mcp":           {"wukath"},
	"models":        {"xuanyang15"},
	"services":      {"DeanChensj", "asugumar-google"},
	"skills":        {"wukath"},
	"tools":         {"xuanyang15", "asugumar-google"},
	"tracing":       {"jawoszek"},
	"web":           {"wyf7107"},
	"workflow":      {"DeanChensj"},
}
