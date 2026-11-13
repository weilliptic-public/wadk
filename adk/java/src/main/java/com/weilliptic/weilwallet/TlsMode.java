package com.weilliptic.weilwallet;

import javax.net.ssl.SSLContext;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;
import java.net.http.HttpClient;
import java.security.cert.X509Certificate;

/**
 * How HTTP clients in this SDK treat TLS certificates.
 *
 * <p>Verification is on by default. A development platform with a self-signed
 * certificate can turn it off, which is the Java counterpart of the Python
 * SDK's {@code verify=False}.</p>
 *
 * <p>Turning it off means the connection is no longer protected against
 * interception: the client will accept any certificate from any server. It is
 * for a local platform you control, never for anything reachable by others.
 * Because the consequence is silent, it cannot be enabled by a stray config
 * file — only by this call, or by setting {@code WEIL_TLS_VERIFY=0} in the
 * environment.</p>
 *
 * <pre>{@code
 * TlsMode.trustAllCertificates();   // once, at startup, for a dev platform
 * }</pre>
 */
public final class TlsMode {

    private static volatile boolean verifyCertificates = resolveDefault();

    private TlsMode() {}

    private static boolean resolveDefault() {
        String env = System.getenv("WEIL_TLS_VERIFY");
        return !"0".equals(env) && !"false".equalsIgnoreCase(env);
    }

    /** Whether certificates are verified. True unless explicitly disabled. */
    public static boolean verifyCertificates() {
        return verifyCertificates;
    }

    /**
     * Stop verifying TLS certificates, process-wide.
     *
     * <p>Only for a local development platform with a self-signed certificate.
     * Prints a warning, because a silent downgrade of transport security is
     * worse than a noisy one.</p>
     */
    public static void trustAllCertificates() {
        verifyCertificates = false;
        System.err.println(
            "WARNING: TLS certificate verification is OFF. Connections are not "
            + "protected against interception. Use this only against a local "
            + "development platform.");
    }

    /** Resume verifying certificates. */
    public static void verifyAllCertificates() {
        verifyCertificates = true;
    }

    /**
     * An HttpClient builder honouring the current TLS mode.
     *
     * <p>Use this instead of {@code HttpClient.newBuilder()} so one setting
     * covers every request the SDK makes.</p>
     */
    public static HttpClient.Builder httpClientBuilder() {
        HttpClient.Builder builder = HttpClient.newBuilder();
        if (verifyCertificates) {
            return builder;
        }
        return builder.sslContext(trustAllContext());
    }

    /** An HttpClient honouring the current TLS mode. */
    public static HttpClient httpClient() {
        return httpClientBuilder().build();
    }

    private static SSLContext trustAllContext() {
        try {
            TrustManager[] trustAll = new TrustManager[] {
                new X509TrustManager() {
                    @Override
                    public void checkClientTrusted(X509Certificate[] chain, String authType) {}

                    @Override
                    public void checkServerTrusted(X509Certificate[] chain, String authType) {}

                    @Override
                    public X509Certificate[] getAcceptedIssuers() {
                        return new X509Certificate[0];
                    }
                }
            };
            SSLContext context = SSLContext.getInstance("TLS");
            context.init(null, trustAll, new java.security.SecureRandom());
            return context;
        } catch (Exception e) {
            throw new IllegalStateException("cannot build a trust-all SSL context", e);
        }
    }
}
