package com.weilliptic.weilwallet;

/**
 * Thrown when a contract ID string is invalid.
 */
public class InvalidContractIdException extends RuntimeException {

    private final String msg;

    public InvalidContractIdException(String msg) {
        super("invalid contract id: " + msg);
        this.msg = msg;
    }

    public String getMsg() {
        return msg;
    }
}
