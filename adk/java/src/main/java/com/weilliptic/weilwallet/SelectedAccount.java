package com.weilliptic.weilwallet;

/**
 * Identifies which account in the wallet is currently active.
 */
public final class SelectedAccount {
    public enum Type { DERIVED, EXTERNAL }

    private final Type type;
    private final int index;

    public SelectedAccount(Type type, int index) {
        this.type = type;
        this.index = index;
    }

    public static SelectedAccount external(int index) {
        return new SelectedAccount(Type.EXTERNAL, index);
    }

    public static SelectedAccount derived(int index) {
        return new SelectedAccount(Type.DERIVED, index);
    }

    public Type getType() {
        return type;
    }

    public int getIndex() {
        return index;
    }
}

