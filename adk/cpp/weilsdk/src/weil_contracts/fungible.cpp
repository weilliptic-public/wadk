/**
 * @file fungible.cpp
 * @brief Implementation of fungible token contract operations
 * @details This file provides implementations for ERC-20-like fungible token operations
 *          including balance queries, transfers, approvals, minting, and allowance management.
 */

#include "weilsdk/weil_contracts/fungible.h"
#include "external/nlohmann.hpp"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/error.h"
#include "weilsdk/ledger.h"
#include "weilsdk/runtime.h"
#include <string>

namespace weilcontracts {

  /**
   * @brief Constructs a FungibleToken with the given name and symbol
   * @param name The name of the token
   * @param symbol The symbol of the token
   */
  FungibleToken::FungibleToken(std::string name, std::string symbol)
      : name(name), symbol(symbol), totalSupply(0), allowances(0) {}

  /**
   * @brief Gets the name of the token
   * @return The token name
   */
  std::string FungibleToken::getName() const { return this->name; }

  /**
   * @brief Gets the symbol of the token
   * @return The token symbol
   */
  std::string FungibleToken::getSymbol() const { return this->symbol; }

  /**
   * @brief Gets the number of decimals for the token
   * @return Always returns 0 for this implementation
   */
  uint8_t FungibleToken::getDecimals() const { return 0; }

  /**
   * @brief Gets the token details (name, symbol, decimals)
   * @return A TokenDetails structure containing the token information
   */
  TokenDetails FungibleToken::getDetails() const {
    return TokenDetails{this->name, this->symbol, getDecimals()};
  }

  /**
   * @brief Gets the total supply of the token
   * @return The total supply amount
   */
  uint64_t FungibleToken::getTotalSupply() const { return this->totalSupply; }

  /**
   * @brief Sets the name of the token
   * @param _name The new name to set
   */
  void FungibleToken::setName(std::string _name) { this->name = _name; }
  
  /**
   * @brief Sets the symbol of the token
   * @param _symbol The new symbol to set
   */
  void FungibleToken::setSymbol(std::string _symbol) { this->symbol = _symbol; }
  
  /**
   * @brief Sets the total supply of the token
   * @param _supply The new total supply amount
   */
  void FungibleToken::setTotalSupply(uint64_t _supply) { this->totalSupply = _supply; }
  
  /**
   * @brief Sets the allowances map for the token
   * @param _allowances The new allowances map to set
   */
  void FungibleToken::setAllowances(
      collections::WeilMap<std::string, uint64_t> _allowances) {
    this->allowances = _allowances;
  }

  /**
   * @brief Gets the balance of a given address for this token
   * @param addr The address to query the balance for
   * @return The balance amount
   */
  uint64_t FungibleToken::balanceFor(std::string addr) {
    return weilsdk::Ledger::balanceFor(addr, getSymbol());
  }

  /**
   * @brief Transfers tokens from the sender to the specified address
   * @param toAddr The address to transfer tokens to
   * @param amount The amount of tokens to transfer
   * @return A pair where the first element indicates success (true) or failure (false),
   *         and the second element contains the result message
   */
  std::pair<bool, std::string> FungibleToken::transfer(std::string toAddr,
                                                      uint64_t amount) {
    return weilsdk::Ledger::transfer(getSymbol(), weilsdk::Runtime::sender(),
                                    toAddr, amount);
  }

  /**
   * @brief Approves a spender to transfer tokens on behalf of the sender
   * @param spender The address that will be approved to spend tokens
   * @param amount The amount of tokens to approve
   */
  void FungibleToken::approve(std::string spender, uint64_t amount) {
    std::string key = weilsdk::Runtime::sender() + "$" + spender;
    this->getAllowances().insert(key, amount);
  }

  /**
   * @brief Mints new tokens to the sender's address
   * @param amount The amount of tokens to mint
   * @return A pair where the first element indicates success (true) or failure (false),
   *         and the second element contains the result message
   */
  std::pair<bool, std::string> FungibleToken::mint(uint64_t amount) {
    uint64_t currentSupply = this->getTotalSupply();
    this->setTotalSupply(currentSupply+amount);
    return weilsdk::Ledger::mint(getSymbol(), weilsdk::Runtime::sender(),
        amount);
  }

  /**
   * @brief Transfers tokens from one address to another on behalf of the sender
   * @details This function allows a spender to transfer tokens from the owner's address
   *          if the spender has been approved for the transfer amount.
   * @param fromAddr The address to transfer tokens from
   * @param toAddr The address to transfer tokens to
   * @param amount The amount of tokens to transfer
   * @return A pair where the first element indicates success (true) or failure (false),
   *         and the second element contains the result message
   */
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

  /**
   * @brief Gets the allowance amount that a spender is approved to transfer from an owner
   * @param owner The address that owns the tokens
   * @param spender The address that is approved to spend tokens
   * @return The allowance amount, or 0 if no allowance exists
   */
  uint64_t FungibleToken::getAllowance(std::string owner, std::string spender) {
    std::string key = owner + "$" + spender;
    if (this->getAllowances().contains(key)) {
      return this->getAllowances().get(key);
    } else
      return 0;
  }
}; // namespace weilcontracts
