package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.CallbackContext;
import com.google.adk.agents.LlmAgent;
import com.google.adk.models.LlmRequest;
import com.google.adk.models.LlmResponse;
import com.google.adk.plugins.BasePlugin;
import com.google.genai.types.Content;
import com.google.genai.types.GenerateContentConfig;

import io.reactivex.rxjava3.core.Maybe;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.time.LocalDate;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.Objects;
import java.util.function.Predicate;

/**
 * The instruction that makes an agent actually use its graph memory.
 *
 * <p>{@link PensieveTools} gives a model the twelve tools; nothing makes it
 * reach for them. Tool descriptions alone produce an agent that remembers
 * occasionally and recalls almost never, because recalling requires acting
 * before the user has said anything that obviously calls for it. This text
 * supplies the discipline: read first, write only the facts and preferences
 * the user states (nothing for a greeting or a plain question), decompose a
 * sentence into entities and relations rather than one blob, and correct
 * rather than accumulate. It is the prompt the icarus agent uses.</p>
 *
 * <p>It is added for you: {@link Weil#plugins} includes {@link #plugin()},
 * which appends this text to the system instruction of every agent holding
 * the memory tools, before each of its model calls. An agent given
 * {@link Weil#withMemory} or {@link Weil#memory} needs nothing more. A plugin
 * list of your own takes {@link #plugin()} beside the {@link WeilPlugin}, and
 * {@link #forAgent()} gives the text for an instruction written by hand.</p>
 *
 * <p>The text lives in {@code memory-prompt.txt} beside this class rather than
 * in a string literal, so the JSON examples in it stay readable and need no
 * Java escaping.</p>
 */
public final class MemoryPrompt {

    private static final String RESOURCE = "memory-prompt.txt";

    /** What the tools are called collectively when the prompt refers to them. */
    public static final String DEFAULT_SERVER = "memory";

    private static volatile String template;

    private MemoryPrompt() {}

    /**
     * A plugin that appends {@link #forAgent()} to the system instruction of
     * every agent holding the memory tools, just before each model call, so
     * the date in it is always that day's. Agents without memory are left
     * alone, and an instruction that already carries the text is not given it
     * twice.
     */
    public static BasePlugin plugin() {
        return plugin(MemoryPrompt::holdsMemoryTools);
    }

    /** As {@link #plugin()}, for the agents {@code holdsMemory} picks. */
    public static BasePlugin plugin(Predicate<LlmAgent> holdsMemory) {
        return new Injector(Objects.requireNonNull(holdsMemory, "holdsMemory"));
    }

    /** Whether {@code agent} holds the memory tools themselves, as {@link Weil#memory} gives them. */
    public static boolean holdsMemoryTools(LlmAgent agent) {
        return agent.toolsUnion().stream().anyMatch(PensieveTools::isMemoryTool);
    }

    /** Adds the prompt to the model calls of the agents it picks. */
    private static final class Injector extends BasePlugin {
        private final Predicate<LlmAgent> holdsMemory;

        Injector(Predicate<LlmAgent> holdsMemory) {
            super("weil-memory-prompt");
            this.holdsMemory = holdsMemory;
        }

        @Override
        public Maybe<LlmResponse> beforeModelCallback(CallbackContext ctx, LlmRequest.Builder request) {
            if (ctx.invocationContext().agent() instanceof LlmAgent agent
                && holdsMemory.test(agent)
                && !carriesPrompt(request)) {
                request.appendInstructions(List.of(forAgent()));
            }
            return Maybe.empty();
        }
    }

    /** Whether the request's system instruction already holds the prompt, put there by hand. */
    private static boolean carriesPrompt(LlmRequest.Builder request) {
        String opening = template();
        opening = opening.substring(0, Math.max(0, opening.indexOf('{')));
        String start = opening;
        return request.config()
            .flatMap(GenerateContentConfig::systemInstruction)
            .flatMap(Content::parts)
            .map(parts -> parts.stream()
                .anyMatch(part -> part.text().map(text -> text.contains(start)).orElse(false)))
            .orElse(false);
    }

    /** The prompt, with today's date and the default tool name. */
    public static String forAgent() {
        return forAgent(DEFAULT_SERVER, LocalDate.now());
    }

    /**
     * The prompt, naming the tools {@code server} and dated {@code today}.
     *
     * @param server what to call the tools in the text, e.g. {@code "memory"}
     * @param today  the date the agent should stamp observations with
     */
    public static String forAgent(String server, LocalDate today) {
        Objects.requireNonNull(server, "server");
        Objects.requireNonNull(today, "today");
        return template()
            .replace("{server}", server)
            .replace("{date}", DateTimeFormatter.ISO_LOCAL_DATE.format(today));
    }

    /** The raw text, placeholders unreplaced. */
    public static String template() {
        String cached = template;
        if (cached == null) {
            synchronized (MemoryPrompt.class) {
                cached = template;
                if (cached == null) {
                    cached = load();
                    template = cached;
                }
            }
        }
        return cached;
    }

    private static String load() {
        try (InputStream in = MemoryPrompt.class.getResourceAsStream(RESOURCE)) {
            if (in == null) {
                throw new IllegalStateException(
                    "memory prompt resource missing: " + RESOURCE
                        + " (expected beside " + MemoryPrompt.class.getName() + ")");
            }
            return new String(in.readAllBytes(), StandardCharsets.UTF_8).trim();
        } catch (IOException e) {
            throw new UncheckedIOException("could not read " + RESOURCE, e);
        }
    }
}
