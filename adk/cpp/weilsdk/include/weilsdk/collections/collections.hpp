/**
 * @file collections.hpp
 * @brief Base interface for all collection types
 * @details This file defines the base Collection interface that all collection
 *          implementations must inherit from. It provides a common interface for
 *          accessing the base state path used for persistent storage.
 */

#ifndef COLLECTIONS_HPP
#define COLLECTIONS_HPP

#include "external/nlohmann.hpp"
#include <string>

namespace collections {
  /**
   * @brief Base interface for all collection types
   * @tparam KeyType The type of keys used in the collection
   * @details This is an abstract base class that defines the common interface
   *          for all collection types. Concrete collection implementations must
   *          inherit from this class and implement the base_state_path() method.
   */
  template <typename KeyType> class Collection {
  public:
    /**
     * @brief Virtual destructor
     */
    virtual ~Collection() = default;

    /**
     * @brief Gets the base state path for the collection
     * @details This method returns the base path used for storing collection data
     *          in persistent storage. The path is used as a prefix for all keys
     *          in the collection.
     * @return The base state path as a string
     */
    virtual std::string base_state_path() const = 0;
  };
} // namespace collections

#endif
