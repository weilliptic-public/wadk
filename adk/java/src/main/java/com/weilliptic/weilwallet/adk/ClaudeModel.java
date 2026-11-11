package com.weilliptic.weilwallet.adk;

import com.anthropic.client.AnthropicClient;
import com.anthropic.core.JsonValue;
import com.anthropic.models.messages.CacheControlEphemeral;
import com.anthropic.models.messages.ContentBlock;
import com.anthropic.models.messages.ContentBlockParam;
import com.anthropic.models.messages.Message;
import com.anthropic.models.messages.MessageCreateParams;
import com.anthropic.models.messages.MessageParam;
import com.anthropic.models.messages.TextBlockParam;
import com.anthropic.models.messages.ToolChoice;
import com.anthropic.models.messages.ToolChoiceAuto;
import com.anthropic.models.messages.ToolResultBlockParam;
import com.anthropic.models.messages.ToolUnion;
import com.anthropic.models.messages.ToolUseBlockParam;
import com.fasterxml.jackson.core.type.TypeReference;
import com.google.adk.JsonBaseModel;
import com.google.adk.models.BaseLlm;
import com.google.adk.models.BaseLlmConnection;
import com.google.adk.models.LlmRequest;
import com.google.adk.models.LlmResponse;
import com.google.common.collect.ImmutableList;
import com.google.genai.types.Content;
import com.google.genai.types.FunctionCall;
import com.google.genai.types.FunctionDeclaration;
import com.google.genai.types.GenerateContentConfig;
import com.google.genai.types.GenerateContentResponseUsageMetadata;
import com.google.genai.types.Part;

import io.reactivex.rxjava3.core.Flowable;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.URI;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/**
 * An ADK model backed by the Anthropic Messages API.
 *
 * <p>A drop-in replacement for {@code com.google.adk.models.Claude}, which
 * recognises only {@code text} and {@code tool_use} content blocks and throws
 * {@code UnsupportedOperationException("Not supported yet.")} on anything else.
 * Current Claude models return a {@code thinking} block ahead of a tool call
 * without being asked to, so that throw takes down the first turn of any agent
 * that uses tools. This class maps the blocks it understands and <em>skips</em>
 * the rest, so a block type added tomorrow costs a debug line rather than the
 * run.</p>
 *
 * <p>Two further differences from ADK's version, both deliberate:</p>
 *
 * <ul>
 *   <li>Parallel tool use is left enabled, so a coordinator can call several
 *       specialists in one turn and merge their answers. ADK disables it.</li>
 *   <li>Blocks whose text is empty are dropped rather than sent back as empty
 *       {@code text} blocks, which the API rejects.</li>
 * </ul>
 *
 * <pre>{@code
 * LlmAgent agent = LlmAgent.builder()
 *     .name("travel_planner")
 *     .model(new ClaudeModel("claude-sonnet-5-5", AnthropicOkHttpClient.fromEnv()))
 *     .build();
 * }</pre>
 */
public class ClaudeModel extends BaseLlm {

    private static final Logger logger = LoggerFactory.getLogger(ClaudeModel.class);

    /**
     * ADK's mapper, not a bare one: genai's generated types hold {@code
     * Optional} fields, which Jackson refuses to serialize without the JDK8
     * module ADK has already registered here.
     */
    private static final com.fasterxml.jackson.databind.ObjectMapper JSON =
        JsonBaseModel.getMapper();

    /** Schema keywords whose value is a map of named sub-schemas. */
    private static final List<String> SCHEMA_MAP_KEYWORDS = List.of(
        "$defs", "defs", "dependentSchemas", "patternProperties", "properties");

    /** Schema keywords whose value is a single sub-schema. */
    private static final List<String> SCHEMA_KEYWORDS = List.of(
        "additionalProperties", "additional_properties", "contains", "else", "if",
        "items", "not", "propertyNames", "then", "unevaluatedProperties");

    /** Schema keywords whose value is a list of sub-schemas. */
    private static final List<String> SCHEMA_LIST_KEYWORDS = List.of(
        "allOf", "all_of", "anyOf", "any_of", "oneOf", "one_of", "prefixItems");

    // Room for a long answer, kept low enough that a request without
    // streaming finishes inside the client's HTTP timeout.
    private static final int DEFAULT_MAX_TOKENS = 16000;

    /** Where the Messages API lives when nothing says otherwise. */
    private static final String DEFAULT_PROVIDER = "api.anthropic.com";

    private final AnthropicClient client;
    private final int maxTokens;
    private final String provider;
    private final boolean promptCaching;

    public ClaudeModel(String modelName, AnthropicClient client) {
        this(modelName, client, DEFAULT_MAX_TOKENS);
    }

    public ClaudeModel(String modelName, AnthropicClient client, int maxTokens) {
        this(modelName, client, maxTokens, providerFromEnv());
    }

    /**
     * @param provider who serves the Messages API for this client, as recorded
     *                 in audit entries; pass it when the client was built with
     *                 an explicit base URL rather than from the environment
     */
    public ClaudeModel(String modelName, AnthropicClient client, int maxTokens, String provider) {
        this(modelName, client, maxTokens, provider, promptCachingFromEnv(provider));
    }

    /**
     * @param promptCaching mark the prompt's fixed start (the tools and the
     *                      system instruction) for Anthropic's prompt cache, so
     *                      later calls that start the same way read it from
     *                      the cache: cheaper and faster. Off for an endpoint
     *                      that does not take {@code cache_control}.
     */
    public ClaudeModel(
        String modelName, AnthropicClient client, int maxTokens, String provider, boolean promptCaching) {
        super(modelName);
        this.client = client;
        this.maxTokens = maxTokens;
        this.provider = provider;
        this.promptCaching = promptCaching;
    }

    /** Whether requests mark their fixed start for Anthropic's prompt cache. */
    public boolean promptCaching() {
        return promptCaching;
    }

    /**
     * CLAUDE_PROMPT_CACHE when it is set; otherwise on for Anthropic's own
     * API and off for any other endpoint, which may refuse the field.
     */
    private static boolean promptCachingFromEnv(String provider) {
        String set = System.getenv("CLAUDE_PROMPT_CACHE");
        if (set != null && !set.isBlank()) {
            return Boolean.parseBoolean(set.trim());
        }
        return DEFAULT_PROVIDER.equals(provider);
    }

    /**
     * Who serves the Messages API: the host of the base URL, such as
     * {@code api.anthropic.com} or {@code api.moonshot.ai} for Kimi.
     */
    public String provider() {
        return provider;
    }

    /**
     * The host {@code AnthropicOkHttpClient.fromEnv()} sends to: the
     * {@code anthropic.baseUrl} property, then {@code ANTHROPIC_BASE_URL}. The
     * client does not expose its base URL, so it is read from where the client
     * read it.
     */
    private static String providerFromEnv() {
        String baseUrl = System.getProperty("anthropic.baseUrl");
        if (baseUrl == null || baseUrl.isBlank()) {
            baseUrl = System.getenv("ANTHROPIC_BASE_URL");
        }
        if (baseUrl == null || baseUrl.isBlank()) {
            return DEFAULT_PROVIDER;
        }
        try {
            String host = URI.create(baseUrl.trim()).getHost();
            return host != null ? host : baseUrl.trim();
        } catch (IllegalArgumentException e) {
            return baseUrl.trim();
        }
    }

    @Override
    public Flowable<LlmResponse> generateContent(LlmRequest llmRequest, boolean stream) {
        // The Messages API is called without streaming; ADK collects the whole
        // turn before it runs a tool either way.
        List<MessageParam> messages = llmRequest.contents().stream()
            .map(this::toMessageParam)
            .filter(Objects::nonNull)
            .toList();

        List<com.anthropic.models.messages.Tool> anthropicTools = new ArrayList<>(
            declarations(llmRequest).stream().map(this::toAnthropicTool).toList());
        String system = systemText(llmRequest);

        MessageCreateParams.Builder params = MessageCreateParams.builder()
            .model(llmRequest.model().orElse(model()))
            .messages(messages)
            .maxTokens(maxTokens);

        // One cache breakpoint at the end of the fixed start. The API caches in
        // the order tools, system, messages, so a breakpoint on the system
        // instruction covers the tools too; with no instruction it goes on the
        // last tool. A prefix under the model's minimum is simply not cached.
        if (promptCaching && !system.isBlank()) {
            params.systemOfTextBlockParams(List.of(TextBlockParam.builder()
                .text(system)
                .cacheControl(CacheControlEphemeral.builder().build())
                .build()));
        } else {
            params.system(system);
            if (promptCaching && !anthropicTools.isEmpty()) {
                int last = anthropicTools.size() - 1;
                anthropicTools.set(last, anthropicTools.get(last).toBuilder()
                    .cacheControl(CacheControlEphemeral.builder().build())
                    .build());
            }
        }
        List<ToolUnion> tools = anthropicTools.stream().map(ToolUnion::ofTool).toList();

        if (!tools.isEmpty()) {
            params.tools(tools);
            // Parallel calls stay available: a coordinator that holds several
            // specialists as tools needs them in one turn to merge the answers.
            params.toolChoice(ToolChoice.ofAuto(ToolChoiceAuto.builder().build()));
        }

        Message message = client.messages().create(params.build());
        logger.debug("Claude response: {}", message);
        return Flowable.just(toLlmResponse(message));
    }

    @Override
    public BaseLlmConnection connect(LlmRequest llmRequest) {
        throw new UnsupportedOperationException(
            "live connection is not supported for Claude models");
    }

    // ── Response ─────────────────────────────────────────────────────────────

    private LlmResponse toLlmResponse(Message message) {
        LlmResponse.Builder response = LlmResponse.builder();
        // The model that actually answered, which can differ from the one
        // asked for when a provider maps an alias.
        String answeredBy = message.model().asString();
        if (answeredBy != null && !answeredBy.isBlank()) {
            response.modelVersion(answeredBy);
        }

        List<Part> parts = new ArrayList<>();
        if (message.content() != null) {
            for (ContentBlock block : message.content()) {
                Part part = toPart(block);
                if (part != null) {
                    parts.add(part);
                }
            }
        }
        response.content(Content.builder()
            .role("model")
            .parts(ImmutableList.copyOf(parts))
            .build());

        if (message.usage() != null) {
            // Anthropic counts cached prompt tokens apart from input_tokens;
            // ADK's prompt count is all of them, with the cached share as
            // cachedContentTokenCount, as Gemini reports it.
            long cacheRead = message.usage().cacheReadInputTokens().orElse(0L);
            long cacheWrite = message.usage().cacheCreationInputTokens().orElse(0L);
            long in = message.usage().inputTokens() + cacheRead + cacheWrite;
            long out = message.usage().outputTokens();
            response.usageMetadata(GenerateContentResponseUsageMetadata.builder()
                .promptTokenCount((int) in)
                .cachedContentTokenCount((int) cacheRead)
                .candidatesTokenCount((int) out)
                .totalTokenCount((int) (in + out))
                .build());
        }
        return response.build();
    }

    /**
     * Map one content block, or return null for a block with nothing for the
     * agent to act on.
     *
     * <p>A model may think before it answers; the thought is not part of the
     * conversation, so it is skipped rather than turned into a Part the agent
     * would print or send back.</p>
     */
    private Part toPart(ContentBlock block) {
        if (block.isText()) {
            String text = block.asText().text();
            return text.isEmpty() ? null : Part.builder().text(text).build();
        }
        if (block.isToolUse()) {
            return Part.builder()
                .functionCall(FunctionCall.builder()
                    .id(block.asToolUse().id())
                    .name(block.asToolUse().name())
                    .args(block.asToolUse()._input()
                        .convert(new TypeReference<Map<String, Object>>() {}))
                    .build())
                .build();
        }
        if (block.isThinking() || block.isRedactedThinking()) {
            return null;
        }
        logger.debug("skipping unsupported content block: {}", block);
        return null;
    }

    // ── Request ──────────────────────────────────────────────────────────────

    private String systemText(LlmRequest llmRequest) {
        return llmRequest.config()
            .flatMap(GenerateContentConfig::systemInstruction)
            .map(instruction -> instruction.parts().orElse(ImmutableList.of()).stream()
                .filter(part -> part.text().isPresent())
                .map(part -> part.text().get())
                .reduce((a, b) -> a + "\n" + b)
                .orElse(""))
            .orElse("");
    }

    /**
     * Every function declaration in the request, from every tool group: a
     * group that holds none, such as Gemini's google_search, comes first in
     * some requests and must not hide the functions after it.
     */
    private List<FunctionDeclaration> declarations(LlmRequest llmRequest) {
        return llmRequest.config()
            .flatMap(GenerateContentConfig::tools)
            .orElse(ImmutableList.of())
            .stream()
            .flatMap(tool -> tool.functionDeclarations().orElse(ImmutableList.of()).stream())
            .toList();
    }

    /** Returns null for a turn with no block the API would accept. */
    private MessageParam toMessageParam(Content content) {
        List<ContentBlockParam> blocks = content.parts().orElse(ImmutableList.of()).stream()
            .map(this::toBlockParam)
            .filter(Objects::nonNull)
            .toList();
        if (blocks.isEmpty()) {
            return null;
        }
        String role = content.role().orElse("");
        return MessageParam.builder()
            .role(role.equals("model") || role.equals("assistant")
                ? MessageParam.Role.ASSISTANT
                : MessageParam.Role.USER)
            .contentOfBlockParams(blocks)
            .build();
    }

    private ContentBlockParam toBlockParam(Part part) {
        if (part.text().isPresent()) {
            String text = part.text().get();
            return text.isEmpty()
                ? null
                : ContentBlockParam.ofText(TextBlockParam.builder().text(text).build());
        }
        if (part.functionCall().isPresent()) {
            FunctionCall call = part.functionCall().get();
            return ContentBlockParam.ofToolUse(ToolUseBlockParam.builder()
                .id(call.id().orElse(""))
                .name(call.name().orElseThrow())
                .input(JsonValue.from(call.args().orElse(Map.of())))
                .build());
        }
        if (part.functionResponse().isPresent()) {
            var response = part.functionResponse().get();
            Map<String, Object> data = response.response().orElse(Map.of());
            Object result = data.get("result");
            return ContentBlockParam.ofToolResult(ToolResultBlockParam.builder()
                .toolUseId(response.id().orElse(""))
                .content(result != null ? String.valueOf(result) : toJson(data))
                .isError(false)
                .build());
        }
        return null;
    }

    private com.anthropic.models.messages.Tool toAnthropicTool(FunctionDeclaration declaration) {
        Map<String, Object> schema;
        if (declaration.parametersJsonSchema().isPresent()) {
            // MCP-style tools carry a raw JSON Schema; pass it through whole so
            // $ref/$defs keep pointing at something.
            schema = JSON.convertValue(
                declaration.parametersJsonSchema().get(),
                new TypeReference<Map<String, Object>>() {});
        } else {
            Map<String, Object> properties = new LinkedHashMap<>();
            Optional<com.google.genai.types.Schema> parameters = declaration.parameters();
            parameters.flatMap(com.google.genai.types.Schema::properties).ifPresent(declared ->
                declared.forEach((name, sub) -> properties.put(
                    name, JSON.convertValue(sub, new TypeReference<Map<String, Object>>() {}))));

            schema = new LinkedHashMap<>();
            schema.put("type", "object");
            schema.put("properties", properties);
            parameters.flatMap(com.google.genai.types.Schema::required)
                .filter(required -> !required.isEmpty())
                .ifPresent(required -> schema.put("required", required));
        }
        lowercaseTypes(schema);

        return com.anthropic.models.messages.Tool.builder()
            .name(declaration.name().orElseThrow())
            .description(declaration.description().orElse(""))
            .inputSchema(toInputSchema(schema))
            .build();
    }

    /**
     * Lowercase every {@code type} keyword in a schema, which genai emits
     * uppercase ("OBJECT") and the Messages API expects lowercase.
     *
     * <p>A {@code type} may also be a union array ({@code ["string", "null"]}),
     * which is traversed rather than cast.</p>
     */
    @SuppressWarnings("unchecked")
    private void lowercaseTypes(Object node) {
        if (node instanceof List<?> list) {
            list.forEach(this::lowercaseTypes);
            return;
        }
        if (!(node instanceof Map)) {
            return;
        }
        Map<String, Object> schema = (Map<String, Object>) node;

        if (schema.get("type") instanceof String type) {
            schema.put("type", type.toLowerCase(Locale.ROOT));
        }
        for (String keyword : SCHEMA_MAP_KEYWORDS) {
            if (schema.get(keyword) instanceof Map<?, ?> children) {
                children.values().forEach(this::lowercaseTypes);
            }
        }
        for (String keyword : SCHEMA_KEYWORDS) {
            lowercaseTypes(schema.get(keyword));
        }
        for (String keyword : SCHEMA_LIST_KEYWORDS) {
            lowercaseTypes(schema.get(keyword));
        }
    }

    /**
     * Carry every top-level schema keyword through, so a schema that leans on
     * {@code $defs} does not arrive with dangling references.
     */
    private com.anthropic.models.messages.Tool.InputSchema toInputSchema(
        Map<String, Object> schema) {
        var builder = com.anthropic.models.messages.Tool.InputSchema.builder();
        schema.forEach((key, value) -> {
            switch (key) {
                // The input schema's type is always "object"; setting it again
                // would send the key twice.
                case "type" -> { }
                case "properties" -> builder.properties(
                    JsonValue.from(value == null ? Map.of() : value));
                case "required" -> {
                    if (value instanceof List<?> required) {
                        builder.required(required.stream()
                            .filter(Objects::nonNull)
                            .map(String::valueOf)
                            .toList());
                    } else {
                        builder.putAdditionalProperty(key, JsonValue.from(value));
                    }
                }
                default -> builder.putAdditionalProperty(key, JsonValue.from(value));
            }
        });
        return builder.build();
    }

    private String toJson(Object value) {
        try {
            return JSON.writeValueAsString(value);
        } catch (Exception e) {
            logger.warn("could not serialize a tool response to JSON", e);
            return String.valueOf(value);
        }
    }
}
