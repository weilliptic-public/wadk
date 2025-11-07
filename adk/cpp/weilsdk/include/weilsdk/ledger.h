#ifndef LEDGER_H
#define LEDGER_H

#include "external/nlohmann.hpp"
#include <memory>
#include <optional>
#include <string>
#include <variant>
#include <vector>

namespace weilsdk {

  // Ledger class
  class Ledger {
  public:
    static bool balanceExistsFor(std::string addr, std::string symbol);
    static uint64_t balanceFor(std::string addr, std::string symbol);
    static std::pair<bool, std::string> mint(std::string symbol,
                                                std::string fromAddr,
                                                uint64_t amount);
    static std::pair<bool, std::string> transfer(std::string symbol,
                                                std::string fromAddr,
                                                std::string toAddr, uint64_t amount);
  };
} // namespace weilsdk

#endif
