package com.weilliptic.weilwallet.api;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.weilliptic.weilwallet.Utils;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Map;

/**
 * Submit transactions to the WeilChain platform.
 * Sends multipart/form-data with field "transaction" and filename "transaction_data".
 */
public final class PlatformApi {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private PlatformApi() {}

    /**
     * Submit and return the parsed TransactionResult.
     */
    public static TransactionResult submitTransaction(
        SubmitTxnRequest payload,
        HttpClient httpClient,
        String baseUrl,
        boolean nonBlocking
    ) throws IOException, InterruptedException {
        Map<String, Object> payloadMap = payload.toPayloadMap();
        byte[] compressed = Utils.compress(payloadMap);
        String url = baseUrl.replaceAll("/$", "") + "/contracts/execute_smartcontract";

        String boundary = "----WeilWalletBoundary" + System.nanoTime();
        byte[] body = buildMultipartBody(boundary, "transaction", "transaction_data", compressed);

        HttpRequest.Builder builder = HttpRequest.newBuilder()
            .uri(URI.create(url))
            .timeout(Duration.ofSeconds(60))
            .header("Content-Type", "multipart/form-data; boundary=" + boundary)
            .POST(HttpRequest.BodyPublishers.ofByteArray(body));

        if (nonBlocking) {
            // this is to fire the transaction without waiting and execute it lazily
            builder.header("x-non-blocking", "true");
        }

        HttpRequest request = builder.build();
        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));

        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            String respBody = response.body();
            if (respBody != null && respBody.length() > 500) respBody = respBody.substring(0, 500);
            throw new RuntimeException("failed to submit the transaction: HTTP " + response.statusCode() + " " + respBody);
        }

        Map<String, Object> data = MAPPER.readValue(response.body(), new TypeReference<Map<String, Object>>() {});
        return TransactionResult.fromJson(data);
    }

    private static byte[] buildMultipartBody(String boundary, String fieldName, String fileName, byte[] fileContent) throws IOException {
        String crlf = "\r\n";
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(("--" + boundary + crlf).getBytes(StandardCharsets.UTF_8));
        out.write(("Content-Disposition: form-data; name=\"" + fieldName + "\"; filename=\"" + fileName + "\"" + crlf).getBytes(StandardCharsets.UTF_8));
        out.write(("Content-Type: application/octet-stream" + crlf + crlf).getBytes(StandardCharsets.UTF_8));
        out.write(fileContent);
        out.write((crlf + "--" + boundary + "--" + crlf).getBytes(StandardCharsets.UTF_8));
        return out.toByteArray();
    }
}
