package com.weilliptic.weilwallet.adk;

import io.a2a.server.TransportMetadata;
import io.a2a.spec.TransportProtocol;

/**
 * Tells the a2a SDK that JSON-RPC is served here.
 *
 * <p>The SDK checks an agent card against the transports it finds through
 * {@link java.util.ServiceLoader}, and only its Quarkus reference servers
 * register one. {@link WeilAgentServer} serves JSON-RPC itself, so it registers
 * this, in {@code META-INF/services/io.a2a.server.TransportMetadata}; without
 * it the card's JSON-RPC interface is rejected at startup.</p>
 */
public final class WeilJsonRpcTransport implements TransportMetadata {

    @Override
    public String getTransportProtocol() {
        return TransportProtocol.JSONRPC.asString();
    }
}
