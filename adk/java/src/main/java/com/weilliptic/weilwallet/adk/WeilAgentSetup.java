package com.weilliptic.weilwallet.adk;

import com.anthropic.client.okhttp.AnthropicOkHttpClient;
import com.google.adk.models.BaseLlm;
import com.google.adk.models.BaseLlmConnection;
import com.google.adk.models.LlmRequest;
import com.google.adk.models.LlmResponse;
import com.google.adk.models.langchain4j.LangChain4j;
import com.google.genai.types.Content;
import com.google.genai.types.Part;

import dev.langchain4j.model.openai.OpenAiChatModel;
import dev.langchain4j.model.openai.OpenAiStreamingChatModel;

import io.reactivex.rxjava3.core.Flowable;

import java.util.ArrayList;
import java.util.List;

/**
 * The model an agent runs on, chosen by MODEL_PROVIDER in the environment, so
 * one agent's code runs on Anthropic or OpenAI without change. The wallet,
 * plugin and chat come from {@link Weil}.
 */
public final class WeilAgentSetup {

    private WeilAgentSetup() {}

    /** Whether MODEL_PROVIDER selects OpenAI rather than the default, Anthropic. */
    public static boolean useOpenAi() {
        String provider = System.getenv().getOrDefault("MODEL_PROVIDER", "anthropic").trim();
        if (provider.equalsIgnoreCase("openai")) {
            return true;
        }
        if (provider.isEmpty() || provider.equalsIgnoreCase("anthropic")) {
            return false;
        }
        throw new IllegalStateException(
            "MODEL_PROVIDER is \"" + provider + "\"; want anthropic or openai");
    }

    /**
     * The model id: OPENAI_MODEL for OpenAI, which has no default, or
     * CLAUDE_MODEL for Anthropic, which defaults to claude-sonnet-5-5.
     */
    public static String modelId() {
        if (useOpenAi()) {
            String model = System.getenv("OPENAI_MODEL");
            if (model == null || model.isBlank()) {
                throw new IllegalStateException(
                    "MODEL_PROVIDER=openai needs OPENAI_MODEL, the model id to call");
            }
            return model.trim();
        }
        return System.getenv().getOrDefault("CLAUDE_MODEL", "claude-sonnet-5-5");
    }

    /**
     * The model every example runs on, chosen by MODEL_PROVIDER.
     *
     * <ul>
     *   <li>{@code anthropic} (default): {@link ClaudeModel} over the Messages
     *       API. Reads ANTHROPIC_API_KEY, and ANTHROPIC_BASE_URL for any
     *       Anthropic-compatible endpoint.</li>
     *   <li>{@code openai}: ADK's LangChain4j adapter over OpenAI's Chat
     *       Completions API. Reads OPENAI_API_KEY and OPENAI_MODEL, and
     *       OPENAI_BASE_URL for any OpenAI-compatible server (vLLM, Ollama,
     *       OpenRouter ...). It holds a streaming model too, for runs in
     *       {@code StreamingMode.SSE}.</li>
     * </ul>
     */
    public static BaseLlm model() {
        if (!useOpenAi()) {
            return new ClaudeModel(modelId(), AnthropicOkHttpClient.fromEnv());
        }
        String apiKey = System.getenv("OPENAI_API_KEY");
        String baseUrl = System.getenv("OPENAI_BASE_URL");
        boolean customServer = baseUrl != null && !baseUrl.isBlank();
        if ((apiKey == null || apiKey.isBlank()) && !customServer) {
            throw new IllegalStateException("MODEL_PROVIDER=openai needs OPENAI_API_KEY");
        }
        // A local server often takes no key, but the client insists on one.
        String key = apiKey == null || apiKey.isBlank() ? "none" : apiKey.trim();
        OpenAiChatModel.OpenAiChatModelBuilder chat = OpenAiChatModel.builder()
            .modelName(modelId())
            .apiKey(key);
        // Used when a run asks for streaming (RunConfig.StreamingMode.SSE);
        // without it the adapter fails every such run.
        OpenAiStreamingChatModel.OpenAiStreamingChatModelBuilder streaming =
            OpenAiStreamingChatModel.builder()
                .modelName(modelId())
                .apiKey(key);
        if (customServer) {
            chat.baseUrl(baseUrl.trim());
            streaming.baseUrl(baseUrl.trim());
        }
        return new WholeTurnStream(LangChain4j.builder()
            .chatModel(chat.build())
            .streamingChatModel(streaming.build())
            .modelName(modelId())
            .build());
    }

    /**
     * Gives ADK's LangChain4j adapter the streamed turn ADK expects.
     *
     * <p>Streaming, the adapter sends every text chunk as a complete response
     * and then each tool call as a response of its own. ADK would store each
     * chunk as a separate reply, and run parallel tool calls as one turn per
     * call. This marks the chunks partial and ends with one response holding
     * the whole text and every tool call, as a call without streaming
     * returns.</p>
     */
    private static final class WholeTurnStream extends BaseLlm {

        private final BaseLlm delegate;

        WholeTurnStream(BaseLlm delegate) {
            super(delegate.model());
            this.delegate = delegate;
        }

        @Override
        public Flowable<LlmResponse> generateContent(LlmRequest request, boolean stream) {
            if (!stream) {
                return delegate.generateContent(request, false);
            }
            return Flowable.defer(() -> {
                StringBuilder text = new StringBuilder();
                List<Part> calls = new ArrayList<>();
                Flowable<LlmResponse> chunks = delegate.generateContent(request, true)
                    .concatMap(response -> {
                        String chunk = "";
                        for (Part part : response.content().flatMap(Content::parts).orElse(List.of())) {
                            if (part.functionCall().isPresent()) {
                                calls.add(part);
                            } else {
                                chunk += part.text().orElse("");
                            }
                        }
                        text.append(chunk);
                        return chunk.isEmpty()
                            ? Flowable.empty()
                            : Flowable.just(LlmResponse.builder()
                                .content(Content.fromParts(Part.fromText(chunk)))
                                .partial(true)
                                .build());
                    });
                return chunks.concatWith(Flowable.defer(() -> {
                    List<Part> whole = new ArrayList<>();
                    if (text.length() > 0 || calls.isEmpty()) {
                        whole.add(Part.fromText(text.toString()));
                    }
                    whole.addAll(calls);
                    return Flowable.just(LlmResponse.builder()
                        .content(Content.builder().role("model").parts(whole).build())
                        .build());
                }));
            });
        }

        @Override
        public BaseLlmConnection connect(LlmRequest request) {
            return delegate.connect(request);
        }
    }
}
