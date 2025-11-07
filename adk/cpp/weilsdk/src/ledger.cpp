/**
 * @file ledger.cpp
 * @brief Implementation of ledger operations for token balance queries, transfers, and minting
 * @details This file provides implementations for interacting with the ledger contract,
 *          including checking balances, transferring tokens, and minting new tokens.
 */

#include "external/nlohmann.hpp"
#include "weilsdk/runtime.h"
#include "weilsdk/ledger.h"
#include <optional>
#include <string>


namespace weilsdk {
  /**
   * @brief Arguments structure for ledger balance query operations
   */
  struct LedgerBalanceMethodArgs {
    std::string addr;    ///< The address to query the balance for
    std::string symbol;  ///< The token symbol to query
  };

  /**
   * @brief Serializes LedgerBalanceMethodArgs to JSON
   * @param j The JSON object to populate
   * @param args The arguments structure to serialize
   */
  void to_json(nlohmann::json &j, const LedgerBalanceMethodArgs &args) {
    j = nlohmann::json{{"addr", args.addr}, {"symbol", args.symbol}};
  }

  /**
   * @brief Deserializes JSON to LedgerBalanceMethodArgs
   * @param j The JSON object to deserialize from
   * @param args The arguments structure to populate
   */
  void from_json(const nlohmann::json &j, LedgerBalanceMethodArgs &args) {
    j.at("addr").get_to(args.addr);
    j.at("symbol").get_to(args.symbol);
  }

  /**
   * @brief Arguments structure for ledger transfer operations
   */
  struct LedgerTransferMethodArgs {
    std::string symbol;    ///< The token symbol to transfer
    std::string from_addr; ///< The address to transfer from
    std::string to_addr;   ///< The address to transfer to
    uint64_t amount;       ///< The amount to transfer
  };

  /**
   * @brief Serializes LedgerTransferMethodArgs to JSON
   * @param j The JSON object to populate
   * @param args The arguments structure to serialize
   */
  void to_json(nlohmann::json &j, const LedgerTransferMethodArgs &args) {
    j = nlohmann::json{{"symbol", args.symbol},
                      {"from_addr", args.from_addr},
                      {"to_addr", args.to_addr},
                      {"amount", args.amount}};
  }

  /**
   * @brief Deserializes JSON to LedgerTransferMethodArgs
   * @param j The JSON object to deserialize from
   * @param args The arguments structure to populate
   */
  void from_json(const nlohmann::json &j, LedgerTransferMethodArgs &args) {
    j.at("symbol").get_to(args.symbol);
    j.at("from_addr").get_to(args.from_addr);
    j.at("to_addr").get_to(args.to_addr);
    j.at("amount").get_to(args.amount);
  }
  
  /**
   * @brief Arguments structure for ledger mint operations
   */
  struct LedgerMintMethodArgs{
      std::string symbol;  ///< The token symbol to mint
      std::string to_addr; ///< The address to mint tokens to
      uint64_t amount;     ///< The amount to mint
  };
  
  /**
   * @brief Serializes LedgerMintMethodArgs to JSON
   * @param j The JSON object to populate
   * @param args The arguments structure to serialize
   */
  void to_json(nlohmann::json &j, const LedgerMintMethodArgs &args){
      j = nlohmann::json{{"symbol", args.symbol},
                        {"to_addr", args.to_addr},
                        {"amount", args.amount}};   
  }
  
  /**
   * @brief Deserializes JSON to LedgerMintMethodArgs
   * @param j The JSON object to deserialize from
   * @param args The arguments structure to populate
   */
  void from_json(const nlohmann::json &j, LedgerMintMethodArgs &args) {
    j.at("symbol").get_to(args.symbol);
    j.at("to_addr").get_to(args.to_addr);
    j.at("amount").get_to(args.amount);
  }

  /**
   * @brief Checks whether a balance exists for a given address and token symbol
   * @details This function determines if the contract call to fetch the balance
   *          was successful. The first return value of Runtime::callContract indicates
   *          whether there was an error in making the cross-contract call.
   * @param addr The address to check the balance for
   * @param symbol The token symbol to check
   * @return true if the balance query was successful, false otherwise
   */
  bool Ledger::balanceExistsFor(std::string addr, std::string symbol) {
    nlohmann::json j;
    LedgerBalanceMethodArgs l{addr, symbol};
    to_json(j, l);

    std::string serialized_args = j.dump();

    std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
        weilsdk::Runtime::ledgerContractId(), "balance_for", serialized_args);
    return !res.first;
  }

  /**
   * @brief Retrieves the balance for a given address and token symbol
   * @details If fetching the balance returns an error, this function returns
   *          a default balance value of 0.
   * @param addr The address to query the balance for
   * @param symbol The token symbol to query
   * @return The balance amount, or 0 if an error occurred
   */
  uint64_t Ledger::balanceFor(std::string addr, std::string symbol) {
    nlohmann::json j;
    LedgerBalanceMethodArgs l{addr, symbol};
    to_json(j, l);

    std::string serialized_args = j.dump();
    std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
        weilsdk::Runtime::ledgerContractId(), "balance_for", serialized_args);

    if (res.first) {
      // error = true, so default return
      return 0;
    } else {
      return std::stoull(res.second);
    }
  }

  /**
   * @brief Transfers tokens from one address to another
   * @param symbol The token symbol to transfer
   * @param fromAddr The address to transfer tokens from
   * @param toAddr The address to transfer tokens to
   * @param amount The amount of tokens to transfer
   * @return A pair where the first element indicates success (true) or failure (false),
   *         and the second element contains the result message
   */
  std::pair<bool, std::string> Ledger::transfer(std::string symbol, std::string fromAddr, std::string toAddr,
    uint64_t amount) {
    nlohmann::json j;
    LedgerTransferMethodArgs l{symbol, fromAddr, toAddr, amount};
    to_json(j, l);

    std::string serialized_args = j.dump();
    std::string s = weilsdk::Runtime::ledgerContractId();
    
    std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
        weilsdk::Runtime::ledgerContractId(), "transfer", serialized_args);

    // we return whether the transfer was successful
    // which is same as saying whether callContract did not return an error
    return {!res.first, res.second};
  }
  
  /**
   * @brief Mints a specified amount of tokens with the given symbol to the provided address
   * @param symbol The token symbol to mint
   * @param toAddr The address to mint tokens to
   * @param amount The amount of tokens to mint
   * @return A pair where the first element indicates success (true) or failure (false),
   *         and the second element contains the result message
   */
  std::pair<bool, std::string> Ledger::mint(std::string symbol, std::string toAddr, uint64_t amount){
      nlohmann::json j;  
      LedgerMintMethodArgs l{symbol,toAddr, amount};
      to_json(j,l);
      
      std::string serialized_args = j.dump();
      std::string s = weilsdk::Runtime::ledgerContractId();
      
      std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
          weilsdk::Runtime::ledgerContractId(), "mint", serialized_args
      );
      
      // we return whether the mint was successful
      // which is same as saying whether callContract did not return an error
      return {!res.first, res.second};
  }
} // namespace weilsdk