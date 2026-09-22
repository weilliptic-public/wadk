package com.weilliptic.weilwallet;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.bitcoinj.core.ECKey;
import org.bitcoinj.crypto.ChildNumber;
import org.bitcoinj.crypto.DeterministicKey;
import org.bitcoinj.crypto.HDKeyDerivation;
import org.bitcoinj.params.MainNetParams;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

/**
 * Multi-account secp256k1 wallet for the WeilChain platform.
 *
 * <p>Loaded from a {@code wallet.wc} file. Holds:
 * <ul>
 *   <li><b>derivedAccounts</b> — HD-derived from the {@code xprv} stored in the file.</li>
 *   <li><b>addedAccounts</b> — externally imported accounts with their own secret keys.</li>
 * </ul>
 * All signing and address operations act on the currently selected account.
 * Use {@link #setIndex(SelectedAccount)} to switch accounts at runtime.</p>
 */
public final class Wallet {

    private static final ObjectMapper JSON = new ObjectMapper();

    private final List<Account> derivedAccounts = new ArrayList<>();
    private final List<Account> addedAccounts = new ArrayList<>();
    private SelectedAccount currentIndex;
    private OrgInfo org;

    // ── Internal constructor ─────────────────────────────────────────────────

    private Wallet() {}

    /**
     * Create a Wallet from a private key and pre-minted account address.
     *
     * <p>Used by {@code derived} package and callers that hold raw keys.
     * The account is stored as {@code External(0)} and selected by default.</p>
     */
    public Wallet(PrivateKey privateKey, String accountAddress) {
        addedAccounts.add(Account.fromPrivateKeyAndAddress(privateKey, accountAddress));
        currentIndex = SelectedAccount.External(0);
    }

    // ── Static factory ───────────────────────────────────────────────────────

    /**
     * Load a Wallet from a {@code wallet.wc} file.
     *
     * <p>Derived account secret keys are re-derived from the stored {@code xprv}.
     * External account secret keys are read directly from the file.
     * The active account is taken from the {@code selected_account} field
     * (defaults to derived index 0 when absent).</p>
     *
     * @param path Path to the wallet.wc file.
     * @return A fully initialised Wallet.
     * @throws IOException              If the file cannot be read.
     * @throws IllegalArgumentException If the file type is not {@code "wallet"} or
     *                                  contains no accounts.
     */
    public static Wallet fromWalletFile(String path) throws IOException {
        return fromWalletFile(Paths.get(path));
    }

    /**
     * Load a Wallet from a {@code wallet.wc} file.
     *
     * @param path Path to the wallet.wc file.
     * @return A fully initialised Wallet.
     * @throws IOException If the file cannot be read or parsed.
     */
    public static Wallet fromWalletFile(Path path) throws IOException {
        String content = new String(Files.readAllBytes(path));
        JsonNode root = JSON.readTree(content);

        String type = root.path("type").asText("");
        if (!"wallet".equals(type)) {
            throw new IllegalArgumentException(
                "expected file type 'wallet', got '" + type + "'");
        }

        JsonNode derivedNodes   = root.path("derived_accounts");
        JsonNode externalNodes  = root.path("external_accounts");

        if ((derivedNodes.isMissingNode() || derivedNodes.size() == 0)
                && (externalNodes.isMissingNode() || externalNodes.size() == 0)) {
            throw new IllegalArgumentException("wallet file contains no accounts");
        }

        String xprvStr = root.path("xprv").asText();
        DeterministicKey masterKey = DeterministicKey.deserializeB58(xprvStr, MainNetParams.get());
        DeterministicKey accountKey = resolveAccountLevelKey(masterKey, derivedNodes);

        Wallet w = new Wallet();

        for (JsonNode entry : derivedNodes) {
            int index = entry.path("index").asInt();
            String address = entry.path("account_address").asText();
            DeterministicKey childKey = HDKeyDerivation.deriveChildKey(
                accountKey, new ChildNumber(index, false));
            ECKey ecKey = ECKey.fromPrivate(childKey.getPrivKeyBytes());
            w.derivedAccounts.add(new Account(ecKey, address));
        }

        for (JsonNode entry : externalNodes) {
            String secretKeyHex = entry.path("secret_key").asText();
            String address = entry.path("account_address").asText();
            PrivateKey pk = new PrivateKey(secretKeyHex);
            w.addedAccounts.add(Account.fromPrivateKeyAndAddress(pk, address));
        }

        // Resolve selected_account (default: derived index 0).
        String kind = "derived";
        int index = 0;
        JsonNode sel = root.path("selected_account");
        if (!sel.isMissingNode()) {
            kind  = sel.path("type").asText("derived");
            index = sel.path("index").asInt(0);
        }

        if ("external".equals(kind)) {
            if (index >= w.addedAccounts.size()) {
                throw new IllegalArgumentException(
                    "selected external account index " + index
                        + " out of bounds (have " + w.addedAccounts.size() + ")");
            }
            w.currentIndex = SelectedAccount.External(index);
        } else {
            if (index >= w.derivedAccounts.size()) {
                throw new IllegalArgumentException(
                    "selected derived account index " + index
                        + " out of bounds (have " + w.derivedAccounts.size() + ")");
            }
            w.currentIndex = SelectedAccount.Derived(index);
        }

        // ── Resolve org ─────────────────────────────────────────────────
        // Priority: per-account v2 orgs > top-level v2 orgs > v1 single org.
        w.org = resolveOrg(root, kind, index);

        return w;
    }

    // ── Account management ───────────────────────────────────────────────────

    /**
     * Switch the active account.
     *
     * @param selected The account selector (e.g. {@code SelectedAccount.Derived(1)}).
     * @throws IndexOutOfBoundsException If the index is out of bounds.
     * @throws IllegalArgumentException  If the kind is unknown.
     */
    public void setIndex(SelectedAccount selected) {
        switch (selected.kind()) {
            case "derived":
                if (selected.index() < 0 || selected.index() >= derivedAccounts.size()) {
                    throw new IndexOutOfBoundsException(
                        "derived account index " + selected.index()
                            + " out of bounds (have " + derivedAccounts.size() + " derived account(s))");
                }
                break;
            case "external":
                if (selected.index() < 0 || selected.index() >= addedAccounts.size()) {
                    throw new IndexOutOfBoundsException(
                        "external account index " + selected.index()
                            + " out of bounds (have " + addedAccounts.size() + " external account(s))");
                }
                break;
            default:
                throw new IllegalArgumentException(
                    "unknown account kind: '" + selected.kind() + "'");
        }
        currentIndex = selected;
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    /** Return the number of HD-derived accounts. */
    public int derivedAccountCount() {
        return derivedAccounts.size();
    }

    /** Return the number of externally imported accounts. */
    public int externalAccountCount() {
        return addedAccounts.size();
    }

    /** Return the currently selected account index. */
    public SelectedAccount currentAccountIndex() {
        return currentIndex;
    }

    /**
     * Return the org info if this wallet has a linked organization, or {@code null} if none.
     */
    public OrgInfo org() {
        return org;
    }

    /**
     * Return the sentinel-minted 72-char hex account address of the current account.
     */
    public String getAddress() {
        return currentAccount().getAddress();
    }

    /**
     * Return the current account's secp256k1 public key (uncompressed 65 bytes for wire format).
     */
    public byte[] getPublicKeyUncompressed() {
        return currentAccount().getPublicKeyUncompressed();
    }

    /**
     * Return the underlying ECKey of the current account for low-level operations.
     */
    public ECKey getECKey() {
        return currentAccount().getSecretKey();
    }

    /**
     * Sign buf with ECDSA secp256k1 using the current account. Message is hashed
     * with SHA-256, then signed. Returns hex-encoded 64-byte compact signature (r || s).
     */
    public String sign(byte[] buf) {
        return currentAccount().sign(buf);
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    private Account currentAccount() {
        if ("derived".equals(currentIndex.kind())) {
            return derivedAccounts.get(currentIndex.index());
        }
        return addedAccounts.get(currentIndex.index());
    }

    /**
     * Resolve the active org from the wallet JSON.
     *
     * <p>Priority:
     * <ol>
     *   <li>Per-account v2 {@code orgs[]} on the selected account entry.</li>
     *   <li>Top-level v2 {@code orgs[]} with {@code active_org} index.</li>
     *   <li>v1 single {@code org} object.</li>
     * </ol>
     */
    private static OrgInfo resolveOrg(JsonNode root, String selectedKind, int selectedIndex) {
        // 1. Per-account v2 orgs on the selected account entry.
        String arrayField = "external".equals(selectedKind)
            ? "external_accounts" : "derived_accounts";
        JsonNode accountArray = root.path(arrayField);
        if (accountArray.isArray() && selectedIndex < accountArray.size()) {
            JsonNode entry = accountArray.get(selectedIndex);
            JsonNode accountOrgs = entry.path("orgs");
            if (accountOrgs.isArray() && accountOrgs.size() > 0) {
                int activeIdx = entry.path("active_org").asInt(0);
                if (activeIdx >= 0 && activeIdx < accountOrgs.size()) {
                    return orgMembershipToOrgInfo(accountOrgs.get(activeIdx));
                }
            }
        }

        // 2. Top-level v2 orgs.
        JsonNode topOrgs = root.path("orgs");
        if (topOrgs.isArray() && topOrgs.size() > 0) {
            int activeIdx = root.path("active_org").asInt(0);
            if (activeIdx >= 0 && activeIdx < topOrgs.size()) {
                return orgMembershipToOrgInfo(topOrgs.get(activeIdx));
            }
        }

        // 3. v1 single org field.
        JsonNode orgNode = root.path("org");
        if (!orgNode.isMissingNode() && orgNode.isObject()) {
            String name = orgNode.path("name").asText(null);
            if (name != null && !name.isEmpty()) {
                String subgroup = orgNode.path("subgroup").isNull()
                    ? null : orgNode.path("subgroup").asText(null);
                String purpose = orgNode.path("purpose").asText("");
                return new OrgInfo(name, subgroup, purpose);
            }
        }

        return null;
    }

    /** Convert a v2 org membership JSON node to an {@link OrgInfo}. */
    private static OrgInfo orgMembershipToOrgInfo(JsonNode node) {
        String orgName = node.path("org").asText("");
        String subgroup = node.path("subgroup").asText("");
        return new OrgInfo(
            orgName,
            subgroup.isEmpty() ? null : subgroup,
            "");
    }

    /**
     * Return the key at the account derivation level.
     *
     * <p>If deriving child 0 directly matches the first entry's stored
     * {@code public_key}, the xprv is already at account level. Otherwise
     * the method traverses {@code m/44'/9345'/0'/0} first.</p>
     */
    private static DeterministicKey resolveAccountLevelKey(
            DeterministicKey master, JsonNode derivedNodes) {

        if (!derivedNodes.isArray() || derivedNodes.size() == 0) {
            return master;
        }

        JsonNode first = derivedNodes.get(0);
        int firstIndex = first.path("index").asInt(0);
        String expectedPk = first.path("public_key").asText("");

        DeterministicKey child = HDKeyDerivation.deriveChildKey(
            master, new ChildNumber(firstIndex, false));
        byte[] compressedPub = child.getPubKeyPoint().getEncoded(true);
        String pkHex = Utils.bytesToHex(compressedPub);

        if (pkHex.equals(expectedPk)) {
            return master;
        }

        // Root xprv — traverse m/44'/9345'/0'/0.
        DeterministicKey key = master;
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(44,   true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(9345, true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(0,    true));
        key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(0,    false));
        return key;
    }
}
