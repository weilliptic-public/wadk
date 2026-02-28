package com.weilliptic.weilwallet.api;

import com.weilliptic.weilwallet.ContractId;

/**
 * User transaction payload for SmartContractExecutor.
 */
public class UserTransaction {
    private String type = "SmartContractExecutor";
    private ContractId contractAddress;
    private String contractMethod = "";
    private String contractInputBytes;
    private boolean shouldHideArgs = true;

    public UserTransaction() {}

    public UserTransaction(String type, ContractId contractAddress, String contractMethod,
                            String contractInputBytes, boolean shouldHideArgs) {
        this.type = type != null ? type : "SmartContractExecutor";
        this.contractAddress = contractAddress;
        this.contractMethod = contractMethod != null ? contractMethod : "";
        this.contractInputBytes = contractInputBytes;
        this.shouldHideArgs = shouldHideArgs;
    }

    public String getType() { return type; }
    public void setType(String type) { this.type = type; }
    public ContractId getContractAddress() { return contractAddress; }
    public void setContractAddress(ContractId contractAddress) { this.contractAddress = contractAddress; }
    public String getContractMethod() { return contractMethod; }
    public void setContractMethod(String contractMethod) { this.contractMethod = contractMethod; }
    public String getContractInputBytes() { return contractInputBytes; }
    public void setContractInputBytes(String contractInputBytes) { this.contractInputBytes = contractInputBytes; }
    public boolean isShouldHideArgs() { return shouldHideArgs; }
    public void setShouldHideArgs(boolean shouldHideArgs) { this.shouldHideArgs = shouldHideArgs; }
}
