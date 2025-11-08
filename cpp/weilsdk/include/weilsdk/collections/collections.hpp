#ifndef COLLECTIONS_HPP
#define COLLECTIONS_HPP

#include "external/nlohmann.hpp"
#include <string>

// Base interface for all collections
namespace collections {
  template <typename KeyType> class Collection {
  public:
    virtual ~Collection() = default;

    // Pure virtual functions to be implemented by concrete collections
    virtual std::string base_state_path() const = 0;
  };
} // namespace collections

#endif
