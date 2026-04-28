package com.weilliptic.weilwallet;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.bitcoinj.core.ECKey;
import org.bitcoinj.core.Sha256Hash;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * secp256k1-backed wallet for the WeilChain platform.
 * Signs with ECDSA secp256k1 over the SHA-256 digest of the input.
 * Signature is 64-byte compact (r || s), hex-encoded.
 */
public final class Wallet {

    private static final int COMPACT_SIG_LEN = 32;
    private static final ObjectMapper JSON = new ObjectMapper();

    private final List<Account> externalAccounts;
    private SelectedAccount current;

    public Wallet(PrivateKey privateKey) {
        this.externalAccounts = new CopyOnWriteArrayList<>();
        ECKey key = ECKey.fromPrivate(privateKey.toBytes());
        this.externalAccounts.add(new Account(key, ""));
        this.current = SelectedAccount.external(0);
    }

    private Wallet(List<Account> externalAccounts, SelectedAccount current) {
        this.externalAccounts = new CopyOnWriteArrayList<>(externalAccounts);
        this.current = current;
    }

    public static Wallet fromAccountExportFile(Path path) throws IOException {
        Account acc = accountFromExportFile(path);
        return new Wallet(List.of(acc), SelectedAccount.external(0));
    }

    public void addAccountFromExportFile(Path path) throws IOException {
        this.externalAccounts.add(accountFromExportFile(path));
    }

    public void setIndex(SelectedAccount selected) {
        if (selected.getType() != SelectedAccount.Type.EXTERNAL) {
            throw new IllegalArgumentException("unsupported account type: " + selected.getType());
        }
        int i = selected.getIndex();
        if (i < 0 || i >= externalAccounts.size()) {
            throw new IllegalArgumentException(
                "external account index " + i + " out of bounds (have " + externalAccounts.size() + " external account(s))"
            );
        }
        this.current = selected;
    }

    public int externalAccountCount() {
        return externalAccounts.size();
    }

    public String getAddress() {
        return currentAccount().getAddress();
    }

    /**
     * Return the account's secp256k1 public key (uncompressed 65 bytes for wire format).
     */
    public byte[] getPublicKeyUncompressed() {
        return currentAccount().getEcKey().getPubKeyPoint().getEncoded(false);
    }

    public ECKey getECKey() {
        return currentAccount().getEcKey();
    }

    /**
     * Sign buf with ECDSA secp256k1. Message is hashed with SHA-256, then signed.
     * Returns hex-encoded 64-byte compact signature (r || s).
     */
    public String sign(byte[] buf) {
        byte[] digest = Utils.hashSha256(buf);
        Sha256Hash hash = Sha256Hash.wrap(digest);
        ECKey.ECDSASignature sig = currentAccount().getEcKey().sign(hash);
        byte[] r = bigIntegerToBytes32(sig.r);
        byte[] s = bigIntegerToBytes32(sig.s);
        byte[] compact = new byte[64];
        System.arraycopy(r, 0, compact, 0, COMPACT_SIG_LEN);
        System.arraycopy(s, 0, compact, COMPACT_SIG_LEN, COMPACT_SIG_LEN);
        return Utils.bytesToHex(compact);
    }

    private Account currentAccount() {
        int i = current.getIndex();
        if (i < 0 || i >= externalAccounts.size()) {
            i = 0;
        }
        return externalAccounts.get(i);
    }

    private static Account accountFromExportFile(Path path) throws IOException {
        String raw = Files.readString(path);
        Map<String, Object> data = JSON.readValue(raw, new TypeReference<Map<String, Object>>() {});
        Object type = data.get("type");
        if (type == null || !"account".equals(type.toString())) {
            throw new IllegalArgumentException("expected export type 'account', got '" + type + "'");
        }
        Object accountObj = data.get("account");
        if (!(accountObj instanceof Map)) {
            throw new IllegalArgumentException("account export missing 'account' object");
        }
        @SuppressWarnings("unchecked")
        Map<String, Object> account = (Map<String, Object>) accountObj;
        String secretKeyHex = account.get("secret_key") != null ? account.get("secret_key").toString() : "";
        String addr = account.get("account_address") != null ? account.get("account_address").toString() : "";
        if (addr.isEmpty()) {
            throw new IllegalArgumentException("account export missing account_address");
        }
        PrivateKey pk = new PrivateKey(secretKeyHex);
        ECKey key = ECKey.fromPrivate(pk.toBytes());
        return new Account(key, addr);
    }

    private static byte[] bigIntegerToBytes32(BigInteger n) {
        byte[] bytes = n.toByteArray();
        if (bytes.length > COMPACT_SIG_LEN) {
            return Arrays.copyOfRange(bytes, bytes.length - COMPACT_SIG_LEN, bytes.length);
        }
        if (bytes.length < COMPACT_SIG_LEN) {
            byte[] padded = new byte[COMPACT_SIG_LEN];
            System.arraycopy(bytes, 0, padded, COMPACT_SIG_LEN - bytes.length, bytes.length);
            return padded;
        }
        return bytes;
    }
}
