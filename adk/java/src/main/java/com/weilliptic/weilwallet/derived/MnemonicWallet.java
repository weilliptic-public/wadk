package com.weilliptic.weilwallet.derived;

import org.bitcoinj.crypto.HDKeyDerivation;
import org.bitcoinj.crypto.MnemonicCode;
import org.bitcoinj.crypto.MnemonicException;
import org.bitcoinj.crypto.ChildNumber;
import org.bitcoinj.core.ECKey;
import org.bitcoinj.crypto.DeterministicKey;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import com.fasterxml.jackson.databind.ObjectMapper;

/**
 * Wallet created from a BIP39 mnemonic with BIP32 derivation.
 * Derives accounts at m/44'/9345'/0'/0/{index}.
 */
public class MnemonicWallet {

    public static final String DERIVATION_PATH = "m/44'/9345'/0'/0";
    private static final int STORED_WALLET_VERSION = 1;

    private final String mnemonic;
    private final DeterministicKey masterKey;
    private final Map<Integer, WalletAccount> accounts = new ConcurrentHashMap<>();

    public MnemonicWallet(String mnemonic, DeterministicKey masterKey) {
        this.mnemonic = mnemonic;
        this.masterKey = masterKey;
    }

    /**
     * Derive the account at the given index (same as server-side).
     */
    public WalletAccount deriveAccount(int index) {
        return accounts.computeIfAbsent(index, this::deriveAccountAt);
    }

    private WalletAccount deriveAccountAt(int index) {
        // masterKey is at m/44'/9345'/0'/0; derive child at index
        DeterministicKey key = HDKeyDerivation.deriveChildKey(masterKey, new ChildNumber(index, false));
        byte[] privBytes = key.getPrivKeyBytes();
        ECKey ecKey = ECKey.fromPrivate(privBytes);
        byte[] pubBytes = ecKey.getPubKeyPoint().getEncoded(false);
        String address = Keccak.pubkeyToDerivedAddress(pubBytes);
        return new WalletAccount(privBytes, pubBytes, address);
    }

    public WalletAccount getAccount(int index) {
        return deriveAccount(index);
    }

    public String getAddress(int index) {
        return deriveAccount(index).getAddress();
    }

    public String getMnemonic() {
        return mnemonic;
    }

    public String getDerivationPath() {
        return DERIVATION_PATH;
    }

    /** Create a wallet from a mnemonic or generate a new one. */
    public static MnemonicWallet createWallet(String mnemonic, boolean generateIfNull) throws MnemonicException.MnemonicLengthException {
        if (mnemonic == null || mnemonic.trim().isEmpty()) {
            if (!generateIfNull) {
                throw new IllegalArgumentException("Either mnemonic or generateIfNull must be provided");
            }
            mnemonic = generateMnemonic24();
        }
        byte[] seed = MnemonicCode.INSTANCE.toSeed(Arrays.asList(mnemonic.trim().split("\\s+")), "");
        DeterministicKey master = HDKeyDerivation.createMasterPrivateKey(seed);
        DeterministicKey derived = derivePath(master, DERIVATION_PATH);
        return new MnemonicWallet(mnemonic.trim(), derived);
    }

    public static MnemonicWallet createWallet() throws MnemonicException.MnemonicLengthException {
        return createWallet(null, true);
    }

    private static String generateMnemonic24() throws MnemonicException.MnemonicLengthException {
        SecureRandom random = new SecureRandom();
        byte[] entropy = new byte[32]; // 256 bits -> 24 words
        random.nextBytes(entropy);
        List<String> words = MnemonicCode.INSTANCE.toMnemonic(entropy);
        return String.join(" ", words);
    }

    /** Derive path from master (e.g. m/44'/9345'/0'/0). */
    private static DeterministicKey derivePath(DeterministicKey master, String path) {
        String[] parts = path.replace("m/", "").split("/");
        DeterministicKey key = master;
        for (String part : parts) {
            if (part.isEmpty()) continue;
            boolean hardened = part.endsWith("'") || part.endsWith("H");
            int num = Integer.parseInt(part.replace("'", "").replace("H", ""));
            if (hardened) {
                num += ChildNumber.HARDENED_BIT;
            }
            key = HDKeyDerivation.deriveChildKey(key, new ChildNumber(num));
        }
        return key;
    }

    private static final ObjectMapper JSON = new ObjectMapper();

    /** Save an encoded, serialized version of this wallet to a file. */
    public void storeWallet(Path path) throws IOException {
        Map<String, Object> payload = Map.of(
            "version", STORED_WALLET_VERSION,
            "mnemonic", mnemonic,
            "derivation_path", DERIVATION_PATH
        );
        String serialized = JSON.writeValueAsString(payload);
        String encoded = Base64.getEncoder().encodeToString(serialized.getBytes(StandardCharsets.UTF_8));
        Files.writeString(path, encoded);
    }

    /** Load a wallet from a file previously saved with storeWallet(). */
    @SuppressWarnings("unchecked")
    public static MnemonicWallet loadWallet(Path path) throws IOException, MnemonicException.MnemonicLengthException {
        String encoded = Files.readString(path).trim();
        String serialized = new String(Base64.getDecoder().decode(encoded), StandardCharsets.UTF_8);
        Map<String, Object> payload = JSON.readValue(serialized, Map.class);
        int version = payload.get("version") instanceof Number ? ((Number) payload.get("version")).intValue() : 0;
        if (version != STORED_WALLET_VERSION) {
            throw new IllegalArgumentException("Unsupported stored wallet version: " + version + "; expected " + STORED_WALLET_VERSION);
        }
        String mnemonicStr = (String) payload.get("mnemonic");
        String derivationPath = payload.containsKey("derivation_path") ? (String) payload.get("derivation_path") : DERIVATION_PATH;
        byte[] seed = MnemonicCode.INSTANCE.toSeed(Arrays.asList(mnemonicStr.trim().split("\\s+")), "");
        DeterministicKey master = HDKeyDerivation.createMasterPrivateKey(seed);
        DeterministicKey derived = derivePath(master, derivationPath);
        return new MnemonicWallet(mnemonicStr.trim(), derived);
    }
}
