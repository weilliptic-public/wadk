package com.weilliptic.weilwallet.examples.mcpfilesystem;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.mcp.AbstractMcpTool;
import com.google.adk.tools.mcp.McpToolset;
import com.google.adk.tools.mcp.StdioServerParameters;

import com.weilliptic.weilwallet.adk.PermissionGate;
import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAction;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * ADK's {@code examples/mcpfilesystem}: a file assistant over the MCP
 * filesystem server, with a Weil identity. Every MCP tool call is audited
 * under the agent's wallet, and every filesystem tool, reading as well as
 * writing, runs only when the wallet holds Execution or Management purpose in
 * {@value #FILES_SERVICE}: the {@code @WeilSecured} of a tool with no Java
 * method to carry it.
 *
 * <p>Needs Node.js, since the server is started with {@code npx}.</p>
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # or WEIL_WALLET_FILE
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   export MCP_FS_ROOT=/tmp/mcp-demo        # optional: the only directory it may touch
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.mcpfilesystem.McpFilesystemAgent
 * </pre>
 *
 * <p>Pass a request as an argument, or let it run the default conversation.</p>
 */
public final class McpFilesystemAgent {

    /** The subgroup a wallet must be permitted in to use the filesystem tools. */
    static final String FILES_SERVICE = "engg.weil";

    public static void main(String[] args) throws Exception {
        // Resolved, as the server resolves it: on macOS /tmp is /private/tmp,
        // and a path given to the model in the other form would be refused.
        Path root = Files.createDirectories(
                Path.of(System.getenv().getOrDefault("MCP_FS_ROOT", "/tmp/mcp-demo")))
            .toRealPath();

        // Every tool the MCP server provides is gated, decided from on-chain
        // state, not the prompt: a refused call never runs, and the model is
        // handed the refusal to explain. Memory tools are not MCP tools, and
        // stay open as everywhere else.
        WeilAction filesGate = PermissionGate.on(FILES_SERVICE, Weil.identity(),
            System.getenv("SENTINEL_HOST"), ev -> ev.tool() instanceof AbstractMcpTool<?>);

        McpToolset filesystem = new McpToolset(StdioServerParameters.builder()
            .command("npx")
            .args(List.of("-y", "@modelcontextprotocol/server-filesystem", root.toString()))
            .build()
            .toServerParameters());

        List<Object> tools = new ArrayList<>(Weil.memory());
        tools.add(filesystem);

        LlmAgent agent = LlmAgent.builder()
            .name("filesystem_agent")
            .model(WeilAgentSetup.model())
            .description("Assistant that performs file operations through the MCP filesystem server.")
            .instruction("""
                You are a file system assistant. Use the provided tools to read, write, search,
                and manage files and directories under %s. Ask clarifying questions when unsure
                about file operations.
                When the user requests that you append to a file, read the current contents, add
                the appended text with a newline if needed, then overwrite the file with the
                combined content.
                If a tool call is refused, explain the reason to the user instead of retrying.
                """.formatted(root))
            .tools(tools)
            .build();

        // Audit trail and run trace as for every agent, plus the
        // filesystem gate: MCP tools have no method to carry @WeilSecured.
        // The UI keeps serving after web returns, so the MCP server is closed
        // when the process stops, not here.
        Runtime.getRuntime().addShutdownHook(new Thread(filesystem::close, "mcp-close"));
        Weil.web(agent, args, filesGate);
    }

    private McpFilesystemAgent() {}
}
