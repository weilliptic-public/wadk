package com.weilliptic.weilwallet;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.bitcoinj.core.ECKey;
import org.bitcoinj.core.Sha256Hash;
import org.bitcoinj.crypto.ChildNumber;
import org.bitcoinj.crypto.DeterministicKey;
import org.bitcoinj.crypto.HDKeyDerivation;
import org.bitcoinj.params.MainNetParams;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/**
 * Multi-account secp256k1-backed wallet for the WeilChain platform.
 *
 * <p>Supports loading from:
 * <ul>
 *   <li>{@code wallet.wc}: multi-account wallet export (derived + external)</li>
 *   <li>{@code account.wc}: legacy single-account export</li>
 * </ul>
 * Signs with ECDSA secp256k1 over the SHA-256 digest of the input.
 * Signature is 64-byte compact (r || s), hex-encoded.</p>
 */
public final class Wallet {

    private static final int COMPACT_SIG_LEN = 32;
    private static final ObjectMapper JSON = new ObjectMapper();

    private final List<Account> derivedAccounts = new ArrayList<>();
    private final List<Account> addedAccounts = new ArrayList<>();
    private SelectedAccount currentIndex;

    private Wallet() {}

    public Wallet(PrivateKey privateKey) {
        ECKey key = ECKey.fromPrivate(privateKey.toBytes());
        this.addedAccounts.add(new Account(key, ""));
        this.currentIndex = SelectedAccount.external(0);
    }

    // ── account.wc (legacy single-account export) ────────────────────────────

    public static Wallet fromAccountExportFile(Path path) throws IOException {
        Account acc = accountFromExportFile(path);
        Wallet w = new Wallet();
        w.addedAccounts.add(acc);
        w.currentIndex = SelectedAccount.external(0);
        return w;
    }

    public void addAccountFromExportFile(Path path) throws IOException {
        this.addedAccounts.add(accountFromExportFile(path));
    }

    // ── wallet.wc (multi-account export) ─────────────────────────────────────

    public static Wallet fromWalletFile(String path) throws IOException {
        return fromWalletFile(Paths.get(path));
    }

    public static Wallet fromWalletFile(Path path) throws IOException {
        String content = Files.readString(path);
        JsonNode root = JSON.readTree(content);

        String type = root.path("type").asText("");
        if (!"wallet".equals(type)) {
            throw new IllegalArgumentException("expected file type 'wallet', got '" + type + "'");
        }

        JsonNode derivedNodes = root.path("derived_accounts");
        JsonNode externalNodes = root.path("external_accounts");
        if ((!derivedNodes.isArray() || derivedNodes.size() == 0) && (!externalNodes.isArray() || externalNodes.size() == 0)) {
            throw new IllegalArgumentException("wallet file contains no accounts");
        }

        String xprvStr = root.path("xprv").asText();
        DeterministicKey masterKey = DeterministicKey.deserializeB58(xprvStr, MainNetParams.get());
        DeterministicKey accountKey = resolveAccountLevelKey(masterKey, derivedNodes);

        Wallet w = new Wallet();
        for (JsonNode entry : derivedNodes) {
            int index = entry.path("index").asInt();
            String address = entry.path("account_address").asText();
            DeterministicKey childKey = HDKeyDerivation.deriveChildKey(accountKey, new ChildNumber(index, false));
            ECKey ecKey = ECKey.fromPrivate(childKey.getPrivKeyBytes());
            w.derivedAccounts.add(new Account(ecKey, address));
        }
        for (JsonNode entry : externalNodes) {
            String secretKeyHex = entry.path("secret_key").asText();
            String address = entry.path("account_address").asText();
            PrivateKey pk = new PrivateKey(secretKeyHex);
            ECKey ecKey = ECKey.fromPrivate(pk.toBytes());
            w.addedAccounts.add(new Account(ecKey, address));
        }

        String kind = "derived";
        int index = 0;
        JsonNode sel = root.path("selected_account");
        if (!sel.isMissingNode()) {
            kind = sel.path("type").asText("derived");
            index = sel.path("index").asInt(0);
        }
        if ("external".equals(kind)) {
            if (index >= w.addedAccounts.size()) {
                throw new IllegalArgumentException(
                    "selected external account index " + index + " out of bounds (have " + w.addedAccounts.size() + ")"
                );
            }
            w.currentIndex = SelectedAccount.external(index);
        } else {
            if (index >= w.derivedAccounts.size()) {
                throw new IllegalArgumentException(
                    "selected derived account index " + index + " out of bounds (have " + w.derivedAccounts.size() + ")"
                );
            }
            w.currentIndex = SelectedAccount.derived(index);
        }
        return w;
    }

    // ── Account selection ────────────────────────────────────────────────────

    public void setIndex(SelectedAccount selected) {
        if (selected.getType() == SelectedAccount.Type.DERIVED) {
            int i = selected.getIndex();
            if (i < 0 || i >= derivedAccounts.size()) {
                throw new IllegalArgumentException(
                    "derived account index " + i + " out of bounds (have " + derivedAccounts.size() + " derived account(s))"
                );
            }
        } else if (selected.getType() == SelectedAccount.Type.EXTERNAL) {
            int i = selected.getIndex();
            if (i < 0 || i >= addedAccounts.size()) {
                throw new IllegalArgumentException(
                    "external account index " + i + " out of bounds (have " + addedAccounts.size() + " external account(s))"
                );
            }
        } else {
            throw new IllegalArgumentException("unsupported account type: " + selected.getType());
        }
        this.currentIndex = selected;
    }

    public int externalAccountCount() {
        return addedAccounts.size();
    }

    public int derivedAccountCount() {
        return derivedAccounts.size();
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
        if (currentIndex.getType() == SelectedAccount.Type.DERIVED) {
            return derivedAccounts.get(currentIndex.getIndex());
        }
        return addedAccounts.get(currentIndex.getIndex());
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

    /**
     * Return the key at the account derivation level.
     *
     * <p>If deriving child 0 directly matches the first entry's stored
     * {@code public_key}, the xprv is already at account level. Otherwise
     * traverse {@code m/44'/9345'/0'/0} first.</p>
     */
    private static DeterministicKey resolveAccountLevelKey(DeterministicKey master, JsonNode derivedNodes) {
        if (!derivedNodes.isArray() || derivedNodes.size() == 0) {
            return master;
        }
        JsonNode first = derivedNodes.get(0);
        int firstIndex = first.path("index").asInt(0);
        String expectedPk = first.path("public_key").asText("");

        DeterministicKey child = HDKeyDerivation.deriveChildKey(master, new ChildNumber(firstIndex, false));
        byte[] compressedPub = child.getPubKeyPoint().getEncoded(true);
        String pkHex = Utils.bytesToHex(compressedPub);
        if (pkHex.equals(expectedPk)) {
            return master;
        }

        DeterministicKey key = master;
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(44, true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(9345, true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(0, true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(0, false));
        return key;
    }
}
