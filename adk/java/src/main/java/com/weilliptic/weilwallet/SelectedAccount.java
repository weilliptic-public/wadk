package com.weilliptic.weilwallet;

import java.util.Objects;

/**
 * Identifies which account in the wallet is currently active.
 *
 * <p>Supports both {@code Derived} (HD-derived from the wallet xprv) and
 * {@code External} (externally imported) accounts.</p>
 */
public final class SelectedAccount {

    private final String kind;
    private final int index;

    /**
     * @param kind  The account kind ({@code "external"}).
     * @param index The zero-based index into the account list.
     */
    public SelectedAccount(String kind, int index) {
        this.kind = kind;
        this.index = index;
    }

    /**
     * Create a selector for a BIP32 HD-derived account at the given index.
     *
     * @param index Zero-based index into the derived accounts list.
     * @return A new SelectedAccount targeting the derived account.
     */
    public static SelectedAccount Derived(int index) {
        return new SelectedAccount("derived", index);
    }

    /**
     * Create a selector for an externally imported account at the given index.
     *
     * @param index Zero-based index into the external accounts list.
     * @return A new SelectedAccount targeting the external account.
     */
    public static SelectedAccount External(int index) {
        return new SelectedAccount("external", index);
    }

    public String kind() {
        return kind;
    }

    public int index() {
        return index;
    }

    @Override
    public String toString() {
        return kind.substring(0, 1).toUpperCase() + kind.substring(1) + " Account " + index;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof SelectedAccount)) return false;
        SelectedAccount that = (SelectedAccount) o;
        return index == that.index && Objects.equals(kind, that.kind);
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind, index);
    }
}
