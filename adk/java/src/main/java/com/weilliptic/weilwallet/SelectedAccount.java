package com.weilliptic.weilwallet;

/**
 * Identifies which account in the wallet is currently active.
 *
 * <p>Use the factory methods {@link #derived(int)} and {@link #external(int)}
 * to create instances, then pass to {@link Wallet#setIndex(SelectedAccount)} or
 * {@link WeilClient#setAccount(SelectedAccount)} to switch the signing account.</p>
 */
public final class SelectedAccount {
    /** The kind of account: BIP32-derived from the wallet's xprv, or externally imported. */
    public enum Type { DERIVED, EXTERNAL }

    private final Type type;
    private final int index;

    /**
     * Construct a SelectedAccount directly.
     * Prefer the factory methods {@link #derived(int)} and {@link #external(int)}.
     *
     * @param type  account kind (DERIVED or EXTERNAL).
     * @param index zero-based position in the corresponding account list.
     */
    public SelectedAccount(Type type, int index) {
        this.type = type;
        this.index = index;
    }

    /**
     * Create a selector for the external (imported) account at {@code index}.
     *
     * @param index zero-based index into the wallet's external account list.
     */
    public static SelectedAccount external(int index) {
        return new SelectedAccount(Type.EXTERNAL, index);
    }

    /**
     * Create a selector for the BIP32-derived account at {@code index}.
     *
     * @param index zero-based index into the wallet's derived account list.
     */
    public static SelectedAccount derived(int index) {
        return new SelectedAccount(Type.DERIVED, index);
    }

    /**
     * Return the account kind (DERIVED or EXTERNAL).
     */
    public Type getType() {
        return type;
    }

    /**
     * Return the zero-based index within the corresponding account list.
     */
    public int getIndex() {
        return index;
    }
}

