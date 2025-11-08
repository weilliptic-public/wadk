#include "weilsdk/weil_contracts/fungible.h"
#include "external/nlohmann.hpp"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/error.h"
#include "weilsdk/ledger.h"
#include "weilsdk/runtime.h"
#include <string>

namespace weilcontracts {

  FungibleToken::FungibleToken(std::string name, std::string symbol)
      : name(name), symbol(symbol), totalSupply(0), allowances(0) {}

  std::string FungibleToken::getName() const { return this->name; }

  std::string FungibleToken::getSymbol() const { return this->symbol; }

  uint8_t FungibleToken::getDecimals() const { return 0; }

  TokenDetails FungibleToken::getDetails() const {
    return TokenDetails{this->name, this->symbol, getDecimals()};
  }

  uint64_t FungibleToken::getTotalSupply() const { return this->totalSupply; }

  void FungibleToken::setName(std::string _name) { this->name = _name; }
  void FungibleToken::setSymbol(std::string _symbol) { this->symbol = _symbol; }
  void FungibleToken::setTotalSupply(uint64_t _supply) { this->totalSupply = _supply; }
  void FungibleToken::setAllowances(
      collections::WeilMap<std::string, uint64_t> _allowances) {
    this->allowances = _allowances;
  }

  uint64_t FungibleToken::balanceFor(std::string addr) {
    return weilsdk::Ledger::balanceFor(addr, getSymbol());
  }

  std::pair<bool, std::string> FungibleToken::transfer(std::string toAddr,
                                                      uint64_t amount) {
    return weilsdk::Ledger::transfer(getSymbol(), weilsdk::Runtime::sender(),
                                    toAddr, amount);
  }

  void FungibleToken::approve(std::string spender, uint64_t amount) {
    std::string key = weilsdk::Runtime::sender() + "$" + spender;
    this->getAllowances().insert(key, amount);
  }

  std::pair<bool, std::string> FungibleToken::mint(uint64_t amount) {
    uint64_t currentSupply = this->getTotalSupply();
    this->setTotalSupply(currentSupply+amount);
    return weilsdk::Ledger::mint(getSymbol(), weilsdk::Runtime::sender(),
        amount);
  }

  std::pair<bool, std::string> FungibleToken::transferFrom(std::string fromAddr,
                                                          std::string toAddr,
                                                          uint64_t amount) {
    std::string key = fromAddr + "$" + weilsdk::Runtime::sender();

    uint64_t balance = 0;
    if (this->getAllowances().contains(key)) {
      balance = this->getAllowances().get(key);
    }

    if (balance < amount) {
      std::string err =
          "Allowance balance of sender " + weilsdk::Runtime::sender() + " is " +
          std::to_string(balance) +
          ", which is less than amount transfer request from " + fromAddr;
      return {1, err};
    }

    bool isTransfer =
        weilsdk::Ledger::transfer(getSymbol(), fromAddr, toAddr, amount).first;
    if (isTransfer) {
      // TODO: check if we need to delete the key, balance entry from the WeilMap
      this->getAllowances().insert(key, balance - amount);
      std::string msg = "Transfer successful from " + fromAddr + " to " + toAddr;
      return {0, msg};
    } else {
      return {1, "Transfer failed"};
    }
  }

  uint64_t FungibleToken::getAllowance(std::string owner, std::string spender) {
    std::string key = owner + "$" + spender;
    if (this->getAllowances().contains(key)) {
      return this->getAllowances().get(key);
    } else
      return 0;
  }
}; // namespace weilcontracts
