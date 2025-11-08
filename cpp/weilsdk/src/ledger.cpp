#include "external/nlohmann.hpp"
#include "weilsdk/runtime.h"
#include "weilsdk/ledger.h"
#include <optional>
#include <string>


namespace weilsdk {
  struct LedgerBalanceMethodArgs {
    std::string addr;
    std::string symbol;
  };

  void to_json(nlohmann::json &j, const LedgerBalanceMethodArgs &args) {
    j = nlohmann::json{{"addr", args.addr}, {"symbol", args.symbol}};
  }

  void from_json(const nlohmann::json &j, LedgerBalanceMethodArgs &args) {
    j.at("addr").get_to(args.addr);
    j.at("symbol").get_to(args.symbol);
  }

  struct LedgerTransferMethodArgs {
    std::string symbol;
    std::string from_addr;
    std::string to_addr;
    uint64_t amount;
  };

  void to_json(nlohmann::json &j, const LedgerTransferMethodArgs &args) {
    j = nlohmann::json{{"symbol", args.symbol},
                      {"from_addr", args.from_addr},
                      {"to_addr", args.to_addr},
                      {"amount", args.amount}};
  }

  void from_json(const nlohmann::json &j, LedgerTransferMethodArgs &args) {
    j.at("symbol").get_to(args.symbol);
    j.at("from_addr").get_to(args.from_addr);
    j.at("to_addr").get_to(args.to_addr);
    j.at("amount").get_to(args.amount);
  }
  
  struct LedgerMintMethodArgs{
      std::string symbol;
      std::string to_addr;
      uint64_t amount;
  };
  
  void to_json(nlohmann::json &j, const LedgerMintMethodArgs &args){
      j = nlohmann::json{{"symbol", args.symbol},
                        {"to_addr", args.to_addr},
                        {"amount", args.amount}};   
  }
  
  void from_json(const nlohmann::json &j, LedgerMintMethodArgs &args) {
    j.at("symbol").get_to(args.symbol);
    j.at("to_addr").get_to(args.to_addr);
    j.at("amount").get_to(args.amount);
  }

  // this function returns whether the contract call
  // successfully returned balance for this address

  // the first return value of
  // weilsdk::Runtime::callContract(weilsdk::Runtime::ledgerContractId(),
  // "balance_for", serialized_args) tells us whether there was an error in making
  // the crossContractCall to fetch balance
  bool Ledger::balanceExistsFor(std::string addr, std::string symbol) {
    nlohmann::json j;
    LedgerBalanceMethodArgs l{addr, symbol};
    to_json(j, l);

    std::string serialized_args = j.dump();

    std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
        weilsdk::Runtime::ledgerContractId(), "balance_for", serialized_args);
    return !res.first;
  }

  // if fetching balance_for returned an error
  // we return a default balance value (0 for now)
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


  //intended to retrun whether transfer was successful
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
  
  //mints the 'amount' worth of a particular token with the provided symbol
  //to the provided address
  
  //intended to return whether mint was successful
  std::pair<bool, std::string> Ledger::mint(std::string symbol, std::string toAddr, uint64_t amount){
      nlohmann::json j;  
      LedgerMintMethodArgs l{symbol,toAddr, amount};
      to_json(j,l);
      
      std::string serialized_args = j.dump();
      std::string s = weilsdk::Runtime::ledgerContractId();
      
      std::pair<uint64_t, std::string> res = weilsdk::Runtime::callContract(
          weilsdk::Runtime::ledgerContractId(), "mint", serialized_args
      );
      
      // we return whether the transfer was successful
      // which is same as saying whether callContract did not return an error
      return {!res.first, res.second};
  }
} // namespace weilsdk